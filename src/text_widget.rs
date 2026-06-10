use std::cell::{Cell, RefCell, OnceCell};
use std::marker::PhantomData;
use std::sync::{OnceLock, LazyLock};
use std::rc::Rc;

use gtk::{gio, glib, gdk, pango};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, GString};
use glib::subclass::Signal;
use pango::{Layout, AttrList, Attribute, AttrColor, AttrFloat, AttrInt, Underline, Weight, WrapMode};

use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use url::Url;

use crate::{
    APP_ID,
    info_row::PropType,
    utils::Color
};

//------------------------------------------------------------------------------
// CONST variables
//------------------------------------------------------------------------------
pub const INSTALLED_LABEL: &str = " [INSTALLED]";
pub const LINK_SPACER: &str = "   ";

//------------------------------------------------------------------------------
// STRUCT: TextTag
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TextTag {
    text: String,
    version: Option<String>,
    start: u32,
    end: u32,
}

impl TextTag {
    fn new(text: String, version: Option<&str>, start: u32, end: u32) -> Self {
        Self {
            text,
            version: version.map(ToOwned::to_owned),
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

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(get = Self::text, set = Self::set_text)]
        text: PhantomData<GString>,

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

        pub(super) layout: OnceCell<Layout>,
        pub(super) layout_attributes: RefCell<AttrList>,
        pub(super) layout_max_index: Cell<usize>,

        pub(super) link_fg_color: Cell<(u16, u16, u16, u16)>,
        pub(super) comment_fg_color: Cell<(u16, u16, u16, u16)>,
        pub(super) sel_bg_color: Cell<(u16, u16, u16, u16)>,
        pub(super) sel_focus_bg_color: Cell<(u16, u16, u16, u16)>,

        pub(super) cairo_error_color: Cell<(f64, f64, f64, f64)>,

        pub(super) link_list: RefCell<Vec<TextTag>>,
        pub(super) comment_list: RefCell<Vec<TextTag>>,

        pub(super) active_link_index: Cell<Option<usize>>,

        pub(super) selection_start: Cell<Option<usize>>,
        pub(super) selection_end: Cell<Option<usize>>,

        pub(super) is_selecting: Cell<bool>,
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
        //---------------------------------------
        // Request mode function
        //---------------------------------------
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        //---------------------------------------
        // Measure function
        //---------------------------------------
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

        //---------------------------------------
        // Size allocate function
        //---------------------------------------
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let layout = self.layout.get().unwrap();

            layout.set_width(width * pango::SCALE);

            self.draw_area.allocate(width, height, baseline, None);
        }
    }

    impl TextWidget {
        //---------------------------------------
        // Text property getter/setter
        //---------------------------------------
        fn text(&self) -> GString {
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
                    link_list.push(TextTag::new(text.to_owned(), None, 0, text.len() as u32));
                },
                PropType::Packager => {
                    static EXPR: LazyLock<Regex> = LazyLock::new(|| {
                        Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,6}")
                            .expect("Failed to compile Regex")
                    });

                    if let Some(m) = EXPR.find(text) {
                        link_list.push(TextTag::new(
                            format!("mailto:{}", m.as_str()),
                            None,
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
                                caps.get(1).zip(caps.get(2))
                                    .map(|(m1, m2)| {
                                        TextTag::new(
                                            format!("pkg://{}", m1.as_str()),
                                            Some(m2.as_str()),
                                            m1.start() as u32,
                                            m1.end() as u32
                                        )
                                    })
                            })
                        );

                        let comment_len = INSTALLED_LABEL.len() as u32;

                        comment_list.extend(text.match_indices(INSTALLED_LABEL)
                            .filter_map(|(i, s)| {
                                let start = i as u32;

                                start.checked_add(comment_len)
                                    .map(|end| {
                                        TextTag::new(
                                            s.to_owned(),
                                            None,
                                            start,
                                            end
                                        )
                                    })
                            })
                        );
                    }
                },
                _ => {}
            }

            // Set active link index
            if !link_list.is_empty() {
                self.active_link_index.set(Some(0));
            }

            // Store link/comment lists
            self.link_list.replace(link_list);
            self.comment_list.replace(comment_list);

            // Set pango layout text
            let layout = self.layout.get().unwrap();

            layout.set_text(text);

            // Format pango layout text
            obj.set_layout_attributes();

            // Reset selection
            obj.select_none();

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
    // Update colors helper function
    //---------------------------------------
    fn update_colors(&self) {
        let imp = self.imp();

        // Update pango color variables
        imp.link_fg_color.set(Color::pango_color_from_style("link"));
        imp.comment_fg_color.set(Color::pango_color_from_style("comment"));
        imp.sel_bg_color.set(Color::pango_color_from_style("selection"));
        imp.sel_focus_bg_color.set(Color::pango_color_from_style("selection-focus"));

        // Update cairo error color
        imp.cairo_error_color.set(Color::cairo_color_from_style("error"));
    }

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
            widget.set_layout_attributes();
            widget.imp().draw_area.queue_draw();
        });

        // Focused property notify signal
        self.connect_focused_notify(|widget| {
            widget.imp().draw_area.queue_draw();
        });

        let style_manager = adw::StyleManager::for_display(&self.display());

        // System color scheme signal
        style_manager.connect_dark_notify(clone!(
            #[weak(rename_to = widget)] self,
            move |_| {
                widget.update_colors();

                widget.set_layout_attributes();

                widget.imp().draw_area.queue_draw();
            }
        ));

        // System accent color signal
        style_manager.connect_accent_color_notify(clone!(
            #[weak(rename_to = widget)] self,
            move |_| {
                widget.update_colors();

                widget.set_layout_attributes();

                widget.imp().draw_area.queue_draw();
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
        // Reset selection
        self.select_none();

        // Update colors
        self.update_colors();
    }

    //---------------------------------------
    // Layout format helper functions
    //---------------------------------------
    fn add_attr(&self, list: &AttrList, mut attr: Attribute, start: u32, end: u32) {
        attr.set_start_index(start);
        attr.set_end_index(end);

        list.insert(attr);
    }

    fn add_selection_attrs(&self, attr_list: &AttrList, start: u32, end: u32) {
        let imp = self.imp();

        let (red, green, blue, alpha) = if self.focused() {
            imp.sel_focus_bg_color.get()
        } else {
            imp.sel_bg_color.get()
        };

        self.add_attr(attr_list, AttrColor::new_background(red, green, blue).into(), start, end);
        self.add_attr(attr_list, AttrInt::new_background_alpha(alpha).into(), start, end);
    }

    fn add_active_link_attrs(&self, attr_list: &AttrList) {
        if let Some(link) = self.active_link() {
            let underline = if self.underline_links() {
                Underline::Double
            } else {
                Underline::Single
            };

            self.add_attr(attr_list, AttrInt::new_underline(underline).into(), link.start, link.end);
        }
    }

    fn set_layout_attributes(&self) {
        let imp = self.imp();

        let layout = imp.layout.get().unwrap();

        let link_list = imp.link_list.borrow();
        let comment_list = imp.comment_list.borrow();

        let attr_list = AttrList::new();

        // Add link attributes
        let (red, green, blue, alpha) = imp.link_fg_color.get();

        for link in link_list.as_slice() {
            self.add_attr(&attr_list, AttrColor::new_foreground(red, green, blue).into(), link.start, link.end);
            self.add_attr(&attr_list, AttrInt::new_foreground_alpha(alpha).into(), link.start, link.end);

            if self.underline_links() {
                self.add_attr(&attr_list, AttrInt::new_underline(Underline::Single).into(), link.start, link.end);
            }
        }

        // Add comment attributes
        let (red, green, blue, alpha) = imp.comment_fg_color.get();

        for comment in comment_list.as_slice() {
            self.add_attr(&attr_list, AttrInt::new_weight(Weight::Semibold).into(), comment.start, comment.end);
            self.add_attr(&attr_list, AttrColor::new_foreground(red, green, blue).into(), comment.start, comment.end);
            self.add_attr(&attr_list, AttrInt::new_foreground_alpha(alpha).into(), comment.start, comment.end);
            self.add_attr(&attr_list, AttrFloat::new_scale(0.75).into(), comment.start, comment.end);
        }

        layout.set_attributes(Some(&attr_list));

        // Store attributes
        imp.layout_attributes.replace(attr_list);
    }

    //---------------------------------------
    // Setup layout
    //---------------------------------------
    fn setup_layout(&self) {
        let imp = self.imp();

        // Create pango layout
        let layout = imp.draw_area.create_pango_layout(None);
        layout.set_wrap(WrapMode::Word);
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
                let (sel_start, sel_end) = widget.selection_indices();

                if let Some((start, end)) = sel_start.zip(sel_end)
                    .filter(|&(start, end)| start != end)
                    .map(|(start, end)| (start.min(end), start.max(end))) {
                        widget.add_selection_attrs(&attr_list, start as u32, end as u32);
                    }

                // Update pango layout active link attributes
                if widget.focused() && [PropType::Link, PropType::LinkList, PropType::Packager].contains(&widget.ptype()) {
                    widget.add_active_link_attrs(&attr_list);
                }

                layout.set_attributes(Some(&attr_list));

                // Show pango layout
                let (red, green, blue, alpha) = if widget.ptype() == PropType::Error {
                    imp.cairo_error_color.get()
                } else {
                    let color = widget.color();

                    (
                        f64::from(color.red()),
                        f64::from(color.green()),
                        f64::from(color.blue()),
                        f64::from(color.alpha())
                    )
                };

                context.set_source_rgba(red, green, blue, alpha);
                context.move_to(0.0, 0.0);

                pangocairo::functions::show_layout(context, layout);
            }
        ));
    }

    //---------------------------------------
    // Selection helper functions
    //---------------------------------------
    pub fn selected_text(&self) -> Option<GString> {
        let (sel_start, sel_end) = self.selection_indices();

        let (start, end) = sel_start.zip(sel_end)?;

        self.text().get(start.min(end)..start.max(end)).map(GString::from)
    }

    fn selection_indices(&self) -> (Option<usize>, Option<usize>) {
        let imp = self.imp();

        (imp.selection_start.get(), imp.selection_end.get())
    }

    pub fn select_all(&self) {
        let imp = self.imp();

        imp.selection_start.set(Some(0));
        imp.selection_end.set(Some(self.text().len()));

        self.set_has_selection(true);
        imp.draw_area.queue_draw();
    }

    pub fn select_none(&self) {
        let imp = self.imp();

        imp.selection_start.set(None);
        imp.selection_end.set(None);

        self.set_has_selection(false);
        imp.draw_area.queue_draw();
    }

    fn select_range(&self, start: Option<usize>, end: Option<usize>, redraw: bool) {
        let imp = self.imp();

        imp.selection_start.set(start);
        imp.selection_end.set(end);

        if redraw {
            self.set_has_selection(true);
            imp.draw_area.queue_draw();
        }
    }

    fn mark_selection_end(&self, end: Option<usize>) {
        let imp = self.imp();

        imp.selection_end.set(end);

        if !self.has_selection() {
            self.set_has_selection(true);
        }

        imp.draw_area.queue_draw();
    }

    //---------------------------------------
    // Link helper functions
    //---------------------------------------
    pub fn active_link(&self) -> Option<TextTag> {
        let imp = self.imp();

        let link_list = imp.link_list.borrow();
        let index = imp.active_link_index.get()?;

        link_list.get(index).cloned()
    }

    pub fn select_previous_link(&self) {
        let imp = self.imp();

        if let Some(new_index) = imp.active_link_index.get()
            .and_then(|i| i.checked_sub(1)) {
                imp.active_link_index.set(Some(new_index));

                imp.draw_area.queue_draw();
            }
    }

    pub fn select_next_link(&self) {
        let imp = self.imp();

        let link_list = imp.link_list.borrow();

        if let Some(new_index) = imp.active_link_index.get()
            .and_then(|i| i.checked_add(1))
            .filter(|&i| link_list.get(i)
                .is_some_and(|link| link.end <= imp.layout_max_index.get() as u32)
            ) {
                imp.active_link_index.set(Some(new_index));

                imp.draw_area.queue_draw();
            }
    }

    pub fn handle_link(&self, link: &TextTag) {
        let link_url = link.text.clone();

        if let Ok(url) = Url::parse(&link_url) {
            let scheme = url.scheme();

            if scheme == "pkg" {
                if let Some(pkg_name) = url.domain() {
                    self.emit_by_name::<()>(
                        "package-link",
                        &[&pkg_name, &link.version.clone().unwrap_or_default()]
                    );
                }
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
            if Some(cursor) != imp.draw_area.cursor()
                .and_then(|cursor| cursor.name())
                .as_deref() {
                    imp.draw_area.set_cursor_from_name(Some(cursor));
                }
        }
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        let is_clicked: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let pressed_link: Rc<RefCell<Option<TextTag>>> = Rc::new(RefCell::new(None));

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

        // Mouse drag gesture
        let drag_gesture = gtk::GestureDrag::new();

        let is_clicked_clone = Rc::clone(&is_clicked);

        drag_gesture.connect_drag_begin(clone!(
            #[weak(rename_to = widget)] self,
            move |_, x, y| {
                let imp = widget.imp();

                if widget.link_at_xy(x, y).is_none() {
                    if !is_clicked_clone.get() {
                        let (_, index) = widget.index_at_xy(x, y);

                        // Set selection start without redrawing
                        widget.select_range(Some(index), None, false);
                    }

                    imp.is_selecting.set(true);
                }
            }
        ));

        drag_gesture.connect_drag_update(clone!(
            #[weak(rename_to = widget)] self,
            move |controller, x, y| {
                if let Some((start_x, start_y)) = controller.start_point() {
                    let (_, index) = widget.index_at_xy(start_x + x, start_y + y);

                    // Update selection end
                    widget.mark_selection_end(Some(index));
                }
            }
        ));

        drag_gesture.connect_drag_end(clone!(
            #[weak(rename_to = widget)] self,
            move |_, _, _| {
                let imp = widget.imp();

                // Hide selection if necessary
                let (start, end) = widget.selection_indices();

                if end.is_none() || start == end {
                    widget.select_none();
                }

                imp.is_selecting.set(false);
            }
        ));

        imp.draw_area.add_controller(drag_gesture);

        // Mouse click gesture
        let click_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();

        let pressed_link_clone = Rc::clone(&pressed_link);
        let is_clicked_clone = Rc::clone(&is_clicked);

        click_gesture.connect_pressed(clone!(
            #[weak(rename_to = widget)] self,
            move |_, n, x, y| {
                let link = widget.link_at_xy(x, y);

                if link.is_none() {
                    if n == 2 {
                        // Double click: select word under cursor
                        is_clicked_clone.set(true);

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

                        widget.select_range(Some(start), Some(end), true);
                    } else if n == 3 {
                        // Triple click: select all text
                        is_clicked_clone.set(true);

                        widget.select_all();
                    }
                }

                pressed_link_clone.replace(link);
            }
        ));

        let pressed_link_clone = Rc::clone(&pressed_link);
        let is_clicked_clone = Rc::clone(&is_clicked);

        click_gesture.connect_released(clone!(
            #[weak(rename_to = widget)] self,
            move |_, _, x, y| {
                is_clicked_clone.set(false);

                // Launch link if any
                if let Some(link) = pressed_link_clone.take()
                    .filter(|pressed| widget.link_at_xy(x, y).as_ref() == Some(pressed)) {
                        widget.handle_link(&link);
                    }
            }
        ));

        imp.draw_area.add_controller(click_gesture);
    }
}
