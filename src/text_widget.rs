use std::cell::{Cell, RefCell, OnceCell};
use std::marker::PhantomData;
use std::sync::{OnceLock, LazyLock};

use gtk::{gio, glib, gdk, pango};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use glib::subclass::Signal;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use url::Url;

use crate::{
    APP_ID,
    info_row::PropType,
    utils::PangoUtils
};

//------------------------------------------------------------------------------
// CONST variables
//------------------------------------------------------------------------------
pub const INSTALLED_LABEL: &str = " [INSTALLED]";
pub const LINK_SPACER: &str = "     ";

//------------------------------------------------------------------------------
// STRUCT: Marker
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone)]
struct TextTag {
    text: String,
    version: String,
    start: u32,
    end: u32,
}

impl TextTag {
    fn new(text: &str, version: &str, start: u32, end: u32) -> Self {
        Self {
            text: text.to_owned(),
            version: version.to_owned(),
            start,
            end
        }
    }
}

//------------------------------------------------------------------------------
// MODULE: TextWidget
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::TextWidget)]
    #[template(resource = "/com/github/PacView/ui/text_widget.ui")]
    pub struct TextWidget {
        #[template_child]
        pub(super) draw_area: TemplateChild<gtk::DrawingArea>,
        #[template_child]
        pub(super) popover_menu: TemplateChild<gtk::PopoverMenu>,

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(get = Self::text, set = Self::set_text)]
        text: PhantomData<glib::GString>,

        #[property(get, set)]
        can_expand: Cell<bool>,
        #[property(get, set)]
        expanded: Cell<bool>,
        #[property(get, set)]
        max_lines: Cell<i32>,
        #[property(get, set)]
        line_spacing: Cell<f64>,
        #[property(get, set)]
        underline_links: Cell<bool>,
        #[property(get, set)]
        focused: Cell<bool>,
        #[property(get, set)]
        has_selection: Cell<bool>,

        pub(super) layout: OnceCell<pango::Layout>,
        pub(super) layout_attributes: RefCell<pango::AttrList>,
        pub(super) layout_max_index: Cell<usize>,

        pub(super) link_fg_color: Cell<(u16, u16, u16, u16)>,
        pub(super) comment_fg_color: Cell<(u16, u16, u16, u16)>,
        pub(super) sel_bg_color: Cell<(u16, u16, u16, u16)>,
        pub(super) sel_focus_bg_color: Cell<(u16, u16, u16, u16)>,

        pub(super) cairo_error_color: Cell<(f64, f64, f64, f64)>,

        pub(super) link_list: RefCell<Vec<TextTag>>,
        pub(super) comment_list: RefCell<Vec<TextTag>>,

        pub(super) cursor: RefCell<String>,
        pub(super) pressed_link: RefCell<Option<TextTag>>,

        pub(super) is_selecting: Cell<bool>,
        pub(super) is_clicked: Cell<bool>,
        pub(super) selection_start: Cell<Option<usize>>,
        pub(super) selection_end: Cell<Option<usize>>,

        pub(super) focus_link_index: Cell<Option<usize>>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for TextWidget {
        const NAME: &'static str = "TextWidget";
        type Type = super::TextWidget;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            // Install actions
            Self::install_actions(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TextWidget {
        //---------------------------------------
        // Signals
        //---------------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("package-link")
                        .param_types([
                            String::static_type(),
                            String::static_type()
                        ])
                        .build(),
                ]
            })
        }

        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.bind_gsettings();
            obj.setup_widget();
            obj.setup_layout();
            obj.setup_controllers();
        }

        //---------------------------------------
        // Dispose function
        //---------------------------------------
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for TextWidget {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let layout = self.layout.get().unwrap();

            let measure_layout = layout.copy();

            if orientation == gtk::Orientation::Horizontal {
                measure_layout.set_width(pango::SCALE);

                let width = measure_layout.pixel_size().0;

                (width, width, -1, -1)
            } else {
                if for_size == -1 {
                    // Calculate minimum height
                    measure_layout.set_width(-1);
                } else {
                    // Calculate natural height
                    measure_layout.set_width(for_size * pango::SCALE);
                }

                let obj = self.obj();

                let max_lines = obj.max_lines();
                let total_lines = measure_layout.line_count();
                let layout_text_len = layout.text().len();

                // Set widget can expand property
                obj.set_can_expand(max_lines < total_lines);

                // Calculate pango layout height
                let layout_height = if obj.expanded() {
                    // Set layout max index
                    self.layout_max_index.set(layout_text_len);

                    // Get layout height
                    measure_layout.pixel_size().1
                } else {
                    // Set layout max index
                    let max_index = measure_layout.line_readonly(0.max(max_lines - 1))
                        .map_or(layout_text_len, |line| (line.start_index() + line.length()) as usize);

                    self.layout_max_index.set(max_index);

                    // Get layout height
                    let mut rect = measure_layout.line_readonly(0)
                        .map_or_else(|| pango::Rectangle::new(0, 0, 0, 0), |line| line.extents().1);

                    let n_lines = total_lines.min(max_lines);

                    let line_spacing = (rect.height() as f32 * measure_layout.line_spacing()).round() as i32;

                    rect.set_height(0.max(n_lines - 1) * line_spacing + rect.height());

                    pango::extents_to_pixels(Some(&mut rect), None);

                    rect.height()
                };

                // Note: add 2 to ensure double underline visible on last line
                (layout_height + 2, layout_height + 2, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let layout = self.layout.get().unwrap();

            layout.set_width(width * pango::SCALE);

            self.draw_area.allocate(width, height, baseline, None);
        }
    }

    impl TextWidget {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Selection actions
            klass.install_action("text.select-all", None, |widget, _, _| {
                let imp = widget.imp();

                imp.selection_start.set(Some(0));
                imp.selection_end.set(Some(widget.text().len()));

                widget.set_has_selection(true);

                imp.draw_area.queue_draw();
            });

            klass.install_action("text.select-none", None, |widget, _, _| {
                let imp = widget.imp();

                imp.selection_start.set(None);
                imp.selection_end.set(None);

                widget.set_has_selection(false);

                imp.draw_area.queue_draw();
            });

            // Copy action
            klass.install_action("text.copy", None, |widget, _, _| {
                if let Some(text) = widget.selected_text() {
                    widget.clipboard().set_text(&text);
                }
            });

            // Expand/contract actions
            klass.install_action("text.expand", None, |widget, _, _| {
                if widget.can_expand() && !widget.expanded() {
                    widget.set_expanded(true);
                }
            });

            klass.install_action("text.contract", None, |widget, _, _| {
                if widget.can_expand() && widget.expanded() {
                    widget.set_expanded(false);
                }
            });

            // Link actions
            klass.install_action("text.previous-link", None, |widget, _, _| {
                let imp = widget.imp();

                if let Some(new_index) = imp.focus_link_index.get()
                    .and_then(|i| i.checked_sub(1)) {
                        imp.focus_link_index.set(Some(new_index));

                        imp.draw_area.queue_draw();
                    }
            });

            klass.install_action("text.next-link", None, |widget, _, _| {
                let imp = widget.imp();

                let link_list = imp.link_list.borrow();

                if let Some(new_index) = imp.focus_link_index.get()
                    .and_then(|i| i.checked_add(1))
                    .filter(|&i| link_list.get(i)
                        .is_some_and(|link| link.end <= imp.layout_max_index.get() as u32)
                    ) {
                        imp.focus_link_index.set(Some(new_index));

                        imp.draw_area.queue_draw();
                    }
            });

            klass.install_action("text.activate-link", None, |widget, _, _| {
                if let Some(focus_link) = widget.focus_link() {
                    widget.handle_link(&focus_link);
                }
            });
        }

        //---------------------------------------
        // Layout format helper functions
        //---------------------------------------
        fn link_attributes(&self) -> pango::AttrList {
            let attr_list = pango::AttrList::new();

            let (red, green, blue, alpha) = self.link_fg_color.get();

            let link_list = self.link_list.borrow();

            for link in link_list.as_slice() {
                let mut attr = pango::AttrColor::new_foreground(red, green, blue);
                attr.set_start_index(link.start);
                attr.set_end_index(link.end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_foreground_alpha(alpha);
                attr.set_start_index(link.start);
                attr.set_end_index(link.end);

                attr_list.insert(attr);

                if self.obj().underline_links() {
                    let mut attr = pango::AttrInt::new_underline(pango::Underline::Single);
                    attr.set_start_index(link.start);
                    attr.set_end_index(link.end);

                    attr_list.insert(attr);
                }
            }

            attr_list
        }

        fn comment_attributes(&self) -> pango::AttrList {
            let attr_list = pango::AttrList::new();

            let (red, green, blue, alpha) = self.comment_fg_color.get();

            let comment_list = self.comment_list.borrow();

            for comment in comment_list.as_slice() {
                let mut attr = pango::AttrColor::new_foreground(red, green, blue);
                attr.set_start_index(comment.start);
                attr.set_end_index(comment.end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_foreground_alpha(alpha);
                attr.set_start_index(comment.start);
                attr.set_end_index(comment.end);

                attr_list.insert(attr);

                let mut attr = pango::AttrFloat::new_scale(0.9);
                attr.set_start_index(comment.start);
                attr.set_end_index(comment.end);

                attr_list.insert(attr);
            }

            attr_list
        }

        pub(super) fn set_layout_attributes(&self) {
            let layout = self.layout.get().unwrap();

            let attr_list = pango::AttrList::new();

            // Add font weight attribute
            let weight = if self.ptype.get() == PropType::Title {
                pango::Weight::Bold
            } else {
                pango::Weight::Normal
            };

            let mut attr = pango::AttrInt::new_weight(weight);
            attr.set_start_index(pango::ATTR_INDEX_FROM_TEXT_BEGINNING);
            attr.set_end_index(pango::ATTR_INDEX_TO_TEXT_END);

            attr_list.insert(attr);

            // Add link attributes
            attr_list.splice(&self.link_attributes(), 0, 0);

            // Add comment attributes
            attr_list.splice(&self.comment_attributes(), 0, 0);

            layout.set_attributes(Some(&attr_list));

            // Store attributes
            self.layout_attributes.replace(attr_list);
        }

        //---------------------------------------
        // Text property getter/setter
        //---------------------------------------
        fn text(&self) -> glib::GString {
            self.layout.get().unwrap().text()
        }

        fn set_text(&self, text: &str) {
            let obj = self.obj();

            let mut text = text;

            // Create link/comment lists
            let mut link_list: Vec<TextTag> = vec![];
            let mut comment_list: Vec<TextTag> = vec![];

            match obj.ptype() {
                PropType::Link => {
                    link_list.push(TextTag::new(text, "", 0, text.len() as u32));
                },
                PropType::Packager => {
                    static EXPR: LazyLock<Regex> = LazyLock::new(|| {
                        Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,6}")
                            .expect("Failed to compile Regex")
                    });

                    if let Some(m) = EXPR.find(text) {
                        link_list.push(TextTag::new(
                            &format!("mailto:{}", m.as_str()),
                            "",
                            m.start() as u32,
                            m.end() as u32
                        ));
                    }
                },
                PropType::LinkList => {
                    if text.is_empty() {
                        text = "None";
                    } else {
                        static EXPR: LazyLock<FancyRegex> = LazyLock::new(|| {
                            FancyRegex::new(&format!(r"(?<=^|{spacer})([a-zA-Z0-9@._+-]+)([><=]*[a-zA-Z0-9@._+-:]*)(?=:|{spacer}|$)", spacer=regex::escape(LINK_SPACER)))
                                .expect("Failed to compile Regex")
                        });

                        link_list.extend(EXPR.captures_iter(text)
                            .flatten()
                            .filter_map(|caps| {
                                if let Some((m1, m2)) = caps.get(1).zip(caps.get(2)) {
                                    Some(TextTag::new(
                                        &format!("pkg://{}", m1.as_str()),
                                        m2.as_str(),
                                        m1.start() as u32,
                                        m1.end() as u32
                                    ))
                                } else {
                                    None
                                }
                            })
                        );

                        let comment_len = INSTALLED_LABEL.len();

                        comment_list.extend(text.match_indices(INSTALLED_LABEL)
                            .map(|(i, s)| {
                                TextTag::new(
                                    s,
                                    "",
                                    i as u32,
                                    i.checked_add(comment_len).map(|i| i as u32).unwrap_or_default()
                                )
                            })
                        );
                    }
                },
                _ => {}
            }

            // Set focused link index
            if !link_list.is_empty() {
                self.focus_link_index.set(Some(0));
            }

            // Store link/comment lists
            self.link_list.replace(link_list);
            self.comment_list.replace(comment_list);

            // Set pango layout text
            let layout = self.layout.get().unwrap();

            layout.set_text(text);

            // Format pango layout text
            self.set_layout_attributes();

            // Reset selection
            self.selection_start.set(None);
            self.selection_end.set(None);

            obj.set_has_selection(false);

            obj.set_expanded(false);
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: TextWidget
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct TextWidget(ObjectSubclass<imp::TextWidget>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl TextWidget {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        // Expanded property notify signal
        self.connect_expanded_notify(|widget| {
            widget.imp().draw_area.queue_resize();
        });

        // Max lines property notify signal
        self.connect_max_lines_notify(|widget| {
            if !widget.expanded() {
                widget.imp().draw_area.queue_resize();
            }
        });

        // Line spacing property notify signal
        self.connect_line_spacing_notify(|widget| {
            let imp = widget.imp();

            let layout = imp.layout.get().unwrap();

            let line_spacing = widget.line_spacing() as f32;

            if line_spacing != layout.line_spacing() {
                layout.set_line_spacing(line_spacing);

                imp.draw_area.queue_resize();
            }
        });

        // Underline links property notify signal
        self.connect_underline_links_notify(|widget| {
            let imp = widget.imp();

            imp.set_layout_attributes();
            imp.draw_area.queue_draw();
        });

        // Focused property notify signal
        self.connect_focused_notify(|widget| {
            widget.imp().draw_area.queue_draw();
        });

        // System color scheme signal
        let style_manager = adw::StyleManager::for_display(&self.display());

        style_manager.connect_dark_notify(clone!(
            #[weak(rename_to = widget)] self,
            move |_| {
                widget.update_pango_colors();
            }
        ));

        // System accent color signal
        style_manager.connect_accent_color_notify(clone!(
            #[weak(rename_to = widget)] self,
            move |_| {
                widget.update_pango_colors();
            }
        ));
    }

    //---------------------------------------
    // Bind gsettings
    //---------------------------------------
    fn bind_gsettings(&self) {
        let settings = gio::Settings::new(APP_ID);

        settings.bind("property-max-lines", self, "max-lines")
            .get()
            .build();

        settings.bind("property-line-spacing", self, "line-spacing")
            .get()
            .build();

        settings.bind("underline-links", self, "underline-links")
            .get()
            .build();
    }

    //---------------------------------------
    // Setup widget
    //---------------------------------------
    fn setup_widget(&self) {
        let fc = |color: u16| -> f64 {
            f64::from(color)/f64::from(u16::MAX)
        };

        let imp = self.imp();

        // Reset selection
        imp.selection_start.set(None);
        imp.selection_end.set(None);

        // Initialize pango colors
        imp.link_fg_color.set(PangoUtils::color_from_style("link"));
        imp.comment_fg_color.set(PangoUtils::color_from_style("comment"));
        imp.sel_bg_color.set(PangoUtils::color_from_style("selection"));
        imp.sel_focus_bg_color.set(PangoUtils::color_from_style("selection-focus"));

        // Initialize cairo error color
        let (red, green, blue, alpha) = PangoUtils::color_from_style("error");

        imp.cairo_error_color.set((fc(red), fc(green), fc(blue), fc(alpha)));
    }

    //---------------------------------------
    // Layout format helper functions
    //---------------------------------------
    fn focus_link(&self) -> Option<TextTag> {
        let imp = self.imp();

        let link_list = imp.link_list.borrow();
        let focus_index = imp.focus_link_index.get()?;

        link_list.get(focus_index).cloned()
    }

    fn selection_attributes(&self, start: usize, end: usize) -> pango::AttrList {
        let imp = self.imp();

        let attr_list = pango::AttrList::new();

        let (red, green, blue, alpha) = if self.focused() {
            imp.sel_focus_bg_color.get()
        } else {
            imp.sel_bg_color.get()
        };

        let mut attr = pango::AttrColor::new_background(red, green, blue);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);

        let mut attr = pango::AttrInt::new_background_alpha(alpha);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);

        attr_list
    }

    fn focus_link_attributes(&self) -> pango::AttrList {
        let attr_list = pango::AttrList::new();

        if let Some(link) = self.focus_link() {
            let underline = if self.underline_links() {
                pango::Underline::Double
            } else {
                pango::Underline::Single
            };

            let mut attr = pango::AttrInt::new_underline(underline);
            attr.set_start_index(link.start);
            attr.set_end_index(link.end);

            attr_list.insert(attr);
        }

        attr_list
    }

    //---------------------------------------
    // Setup layout
    //---------------------------------------
    fn setup_layout(&self) {
        let imp = self.imp();

        // Create pango layout
        let layout = imp.draw_area.create_pango_layout(None);
        layout.set_wrap(pango::WrapMode::Word);
        layout.set_line_spacing(1.3);

        imp.layout.set(layout).unwrap();

        // Connect drawing area draw function
        imp.draw_area.set_draw_func(clone!(
            #[weak(rename_to = widget)] self,
            move |_, context, _, _| {
                let imp = widget.imp();

                let layout = imp.layout.get().unwrap();
                let attr_list = imp.layout_attributes.borrow().copy().unwrap();

                // Update pango layout selection attributes
                if let Some((start, end)) = imp.selection_start.get().zip(imp.selection_end.get())
                    .filter(|&(start, end)| start != end)
                    .map(|(start, end)| (start.min(end), start.max(end))) {
                        attr_list.splice(&widget.selection_attributes(start, end), 0, 0);
                    }

                // Update pango layout focus link attributes
                if widget.focused() && [PropType::Link, PropType::LinkList, PropType::Packager].contains(&widget.ptype()) {
                    attr_list.splice(&widget.focus_link_attributes(), 0, 0);
                }

                layout.set_attributes(Some(&attr_list));

                // Show pango layout
                let (red, green, blue, alpha) = if widget.ptype() == PropType::Error {
                    imp.cairo_error_color.get()
                } else {
                    let color = widget.color();

                    (f64::from(color.red()), f64::from(color.green()), f64::from(color.blue()), f64::from(color.alpha()))
                };

                context.set_source_rgba(red, green, blue, alpha);
                context.move_to(0.0, 0.0);

                pangocairo::functions::show_layout(context, layout);
            }
        ));
    }

    //---------------------------------------
    // Action helper functions
    //---------------------------------------
    fn selected_text(&self) -> Option<String> {
        let imp = self.imp();

        let (start, end) = imp.selection_start.get().zip(imp.selection_end.get())?;

        self.text().get(start.min(end)..start.max(end)).map(String::from)
    }

    //---------------------------------------
    // Update pango colors helper function
    //---------------------------------------
    fn update_pango_colors(&self) {
        let fc = |color: u16| -> f64 {
            f64::from(color)/f64::from(u16::MAX)
        };

        let imp = self.imp();

        // Update pango color variables
        imp.link_fg_color.set(PangoUtils::color_from_style("link"));
        imp.comment_fg_color.set(PangoUtils::color_from_style("comment"));
        imp.sel_bg_color.set(PangoUtils::color_from_style("selection"));
        imp.sel_focus_bg_color.set(PangoUtils::color_from_style("selection-focus"));

        // Initialize cairo error color
        let (red, green, blue, alpha) = PangoUtils::color_from_style("error");

        imp.cairo_error_color.set((fc(red), fc(green), fc(blue), fc(alpha)));

        // Format pango layout text
        imp.set_layout_attributes();

        // Redraw widget
        imp.draw_area.queue_draw();
    }

    //---------------------------------------
    // Controller helper functions
    //---------------------------------------
    fn index_at_xy(&self, x: f64, y: f64) -> (bool, usize) {
        let layout = self.imp().layout.get().unwrap();

        let (inside, mut index, trailing) = layout.xy_to_index(
            pango::units_from_double(x), pango::units_from_double(y)
        );

        if trailing > 0 {
            index += 1;
        }

        (inside, index as usize)
    }

    fn link_at_xy(&self, x: f64, y: f64) -> Option<TextTag> {
        let (inside, index) = self.index_at_xy(x, y);

        if inside {
            return self.imp().link_list.borrow().iter()
                .find(|&link| link.start <= index as u32 && link.end > index as u32)
                .cloned()
        }

        None
    }

    fn set_motion_cursor(&self, x: f64, y: f64) {
        let imp = self.imp();

        if !imp.is_selecting.get() {
            // Get cursor
            let cursor = if self.link_at_xy(x, y).is_some() {
                "pointer"
            } else {
                "text"
            };

            // Update cursor if necessary
            if cursor != *imp.cursor.borrow() {
                imp.draw_area.set_cursor_from_name(Some(cursor));

                imp.cursor.replace(cursor.to_owned());
            }
        }
    }

    fn handle_link(&self, link: &TextTag) {
        let link_url = link.text.clone();

        if let Ok(url) = Url::parse(&link_url) {
            let url_scheme = url.scheme();

            if url_scheme == "pkg" {
                let pkg_name = url.domain().unwrap_or_default();

                self.emit_by_name::<()>("package-link", &[&pkg_name, &link.version]);
            } else {
                glib::spawn_future_local(async move {
                    let _ = gio::AppInfo::launch_default_for_uri_future(
                        &link_url,
                        None::<&gio::AppLaunchContext>
                    )
                    .await;
                });
            }
        }
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Mouse motion controller
        let motion_controller = gtk::EventControllerMotion::new();

        motion_controller.connect_enter(clone!(
            #[weak(rename_to = widget)] self,
            move |_, x, y| {
                widget.set_motion_cursor(x, y);
            }
        ));

        motion_controller.connect_motion(clone!(
            #[weak(rename_to = widget)] self,
            move |_, x, y| {
                widget.set_motion_cursor(x, y);
            }
        ));

        imp.draw_area.add_controller(motion_controller);

        // Mouse drag controller
        let drag_controller = gtk::GestureDrag::new();

        drag_controller.connect_drag_begin(clone!(
            #[weak(rename_to = widget)] self,
            move |_, x, y| {
                let imp = widget.imp();

                if widget.link_at_xy(x, y).is_none() {
                    if !imp.is_clicked.get() {
                        let (_, index) = widget.index_at_xy(x, y);

                        imp.selection_start.set(Some(index));
                        imp.selection_end.set(None);
                    }

                    imp.is_selecting.set(true);
                }
            }
        ));

        drag_controller.connect_drag_update(clone!(
            #[weak(rename_to = widget)] self,
            move |controller, x, y| {
                let imp = widget.imp();

                if let Some((start_x, start_y)) = controller.start_point() {
                    let (_, index) = widget.index_at_xy(start_x + x, start_y + y);

                    imp.selection_end.set(Some(index));

                    if !widget.has_selection() {
                        widget.set_has_selection(true);
                    }

                    imp.draw_area.queue_draw();
                }
            }
        ));

        drag_controller.connect_drag_end(clone!(
            #[weak(rename_to = widget)] self,
            move |_, _, _| {
                let imp = widget.imp();

                // Redraw if necessary to hide selection
                let start = imp.selection_start.get();
                let end = imp.selection_end.get();

                if end.is_none() || start == end {
                    imp.selection_start.set(None);
                    imp.selection_end.set(None);

                    widget.set_has_selection(false);

                    imp.draw_area.queue_draw();
                }

                imp.is_selecting.set(false);
            }
        ));

        imp.draw_area.add_controller(drag_controller);

        // Mouse click gesture controller
        let click_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();

        click_gesture.connect_pressed(clone!(
            #[weak(rename_to = widget)] self,
            move |_, n, x, y| {
                let imp = widget.imp();

                let link = widget.link_at_xy(x, y);

                if link.is_none() {
                    if n == 2 {
                        // Double click: select word under cursor and redraw widget
                        imp.is_clicked.set(true);

                        let (_, index) = widget.index_at_xy(x, y);

                        let text = widget.text();
                        let (first, last) = text.split_at_checked(index).unzip();

                        let start = first
                            .and_then(|s| {
                                s.as_bytes().iter().rposition(|&ch| {
                                    ch.is_ascii_whitespace() || ch.is_ascii_punctuation()
                                })
                            })
                            .and_then(|start| start.checked_add(1))
                            .unwrap_or(0);

                        let end = last
                            .and_then(|s| {
                                s.as_bytes().iter().position(|&ch| {
                                    ch.is_ascii_whitespace() || ch.is_ascii_punctuation()
                                })
                            })
                            .and_then(|end| end.checked_add(index))
                            .unwrap_or(text.len());

                        imp.selection_start.set(Some(start));
                        imp.selection_end.set(Some(end));

                        widget.set_has_selection(true);

                        imp.draw_area.queue_draw();
                    } else if n == 3 {
                        // Triple click: select all text and redraw widget
                        imp.is_clicked.set(true);

                        imp.selection_start.set(Some(0));
                        imp.selection_end.set(Some(widget.text().len()));

                        widget.set_has_selection(true);

                        imp.draw_area.queue_draw();
                    }
                }

                imp.pressed_link.replace(link);
            }
        ));

        click_gesture.connect_released(clone!(
            #[weak(rename_to = widget)] self,
            move |_, _, x, y| {
                let imp = widget.imp();

                imp.is_clicked.set(false);

                // Launch link if any
                if let Some(link) = imp.pressed_link.take()
                    .filter(|pressed| widget.link_at_xy(x, y).as_ref() == Some(pressed)) {
                        widget.handle_link(&link);
                    }
            }
        ));

        imp.draw_area.add_controller(click_gesture);
    }

    //---------------------------------------
    // Public popup menu function
    //---------------------------------------
    pub fn popup_menu(&self, x: f64, y: f64) {
        let imp = self.imp();

        // Enable/disable copy action
        self.action_set_enabled("text.copy", self.selected_text().is_some());

        // Show popover menu
        let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

        imp.popover_menu.set_pointing_to(Some(&rect));
        imp.popover_menu.popup();
    }
}
