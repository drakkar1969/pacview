use std::cell::{Cell, RefCell, OnceCell};
use std::sync::OnceLock;

use gtk::{gio, glib, gdk, pango};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use glib::subclass::Signal;

use fancy_regex::Regex as FancyRegex;
use num::ToPrimitive;
use regex::Regex;
use url::Url;

//------------------------------------------------------------------------------
// CONST variables
//------------------------------------------------------------------------------
const EXPAND_MARGIN: i32 = 50;

//------------------------------------------------------------------------------
// ENUM: PropType
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PropType")]
pub enum PropType {
    #[default]
    Text,
    Title,
    Link,
    Packager,
    LinkList,
}

//------------------------------------------------------------------------------
// STRUCT: Marker
//------------------------------------------------------------------------------
struct TextTag {
    text: String,
    start: u32,
    end: u32,
}

impl TextTag {
    fn new(text: &str, start: u32, end: u32) -> Self {
        Self {
            text: text.to_string(),
            start,
            end
        }
    }

    fn text(&self) -> String {
        self.text.to_owned()
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
        _text: RefCell<String>,

        #[property(get, set)]
        can_expand: Cell<bool>,
        #[property(get, set)]
        expanded: Cell<bool>,
        #[property(get, set)]
        max_lines: Cell<i32>,

        #[property(get, set)]
        focused: Cell<bool>,

        pub(super) pango_layout: OnceCell<pango::Layout>,

        pub(super) link_fg_color: Cell<(u16, u16, u16, u16)>,
        pub(super) comment_fg_color: Cell<(u16, u16, u16, u16)>,
        pub(super) sel_bg_color: Cell<(u16, u16, u16, u16)>,
        pub(super) sel_focus_bg_color: Cell<(u16, u16, u16, u16)>,

        pub(super) link_list: RefCell<Vec<TextTag>>,
        pub(super) comment_list: RefCell<Vec<TextTag>>,

        pub(super) cursor: RefCell<String>,
        pub(super) pressed_link_url: RefCell<Option<String>>,

        pub(super) is_selecting: Cell<bool>,
        pub(super) is_clicked: Cell<bool>,
        pub(super) selection_start: Cell<Option<u32>>,
        pub(super) selection_end: Cell<Option<u32>>,

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
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TextWidget {
        //---------------------------------------
        // Custom signals
        //---------------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("package-link")
                        .param_types([String::static_type()])
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

            obj.setup_widget();
            obj.setup_layout();
            obj.setup_actions();
            obj.setup_signals();
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
            let layout = self.pango_layout.get().unwrap();

            let measure_layout = layout.copy();

            if orientation == gtk::Orientation::Horizontal {
                measure_layout.set_width(pango::SCALE);

                (measure_layout.pixel_size().0, measure_layout.pixel_size().0, -1, -1)
            } else {
                let obj = self.obj();

                if for_size != -1 {
                    if obj.can_expand() {
                        measure_layout.set_width((for_size - EXPAND_MARGIN) * pango::SCALE);
                    } else {
                        measure_layout.set_width(for_size * pango::SCALE);
                    }
                }

                (measure_layout.pixel_size().1, measure_layout.pixel_size().1, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let layout = self.pango_layout.get().unwrap();

            layout.set_width(width * pango::SCALE);

            self.draw_area.allocate(width, height, baseline, None);
        }
    }

    impl TextWidget {
        //---------------------------------------
        // Layout format helper functions
        //---------------------------------------
        fn format_weight(&self, attr_list: &pango::AttrList) {
            let obj = self.obj();

            let weight = if obj.ptype() == PropType::Title {
                pango::Weight::Bold
            } else {
                pango::Weight::Normal
            };

            let mut attr = pango::AttrInt::new_weight(weight);
            attr.set_start_index(pango::ATTR_INDEX_FROM_TEXT_BEGINNING);
            attr.set_end_index(pango::ATTR_INDEX_TO_TEXT_END);

            attr_list.insert(attr);
        }

        fn format_links(&self, attr_list: &pango::AttrList) {
            let link_list = &*self.link_list.borrow();

            let (red, green, blue, alpha) = self.link_fg_color.get();

            for link in link_list {
                let mut attr = pango::AttrColor::new_foreground(red, green, blue);
                attr.set_start_index(link.start);
                attr.set_end_index(link.end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_foreground_alpha(alpha);
                attr.set_start_index(link.start);
                attr.set_end_index(link.end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_underline(pango::Underline::Single);
                attr.set_start_index(link.start);
                attr.set_end_index(link.end);

                attr_list.insert(attr);
            }
        }

        fn format_comments(&self, attr_list: &pango::AttrList) {
            let comment_list = &*self.comment_list.borrow();

            let (red, green, blue, alpha) = self.comment_fg_color.get();

            for comment in comment_list {
                let mut attr = pango::AttrColor::new_foreground(red, green, blue);
                attr.set_start_index(comment.start);
                attr.set_end_index(comment.end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_foreground_alpha(alpha);
                attr.set_start_index(comment.start);
                attr.set_end_index(comment.end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_weight(pango::Weight::Medium);
                attr.set_start_index(comment.start);
                attr.set_end_index(comment.end);

                attr_list.insert(attr);

                let mut attr = pango::AttrFloat::new_scale(0.9);
                attr.set_start_index(comment.start);
                attr.set_end_index(comment.end);

                attr_list.insert(attr);
            }
        }

        pub(super) fn format_selection(&self, attr_list: &pango::AttrList, start: u32, end: u32) {
            let (red, green, blue, alpha) = if self.obj().focused() {
                self.sel_focus_bg_color.get()
            } else {
                self.sel_bg_color.get()
            };

            let mut attr = pango::AttrColor::new_background(red, green, blue);
            attr.set_start_index(start);
            attr.set_end_index(end);

            attr_list.insert(attr);

            let mut attr = pango::AttrInt::new_background_alpha(alpha);
            attr.set_start_index(start);
            attr.set_end_index(end);

            attr_list.insert(attr);
        }

        pub(super) fn do_format(&self) {
            let layout = self.pango_layout.get().unwrap();

            let attr_list = pango::AttrList::new();

            // Format text
            self.format_weight(&attr_list);

            // Format links
            self.format_links(&attr_list);

            // Format comments
            self.format_comments(&attr_list);

            layout.set_attributes(Some(&attr_list));
        }

        //---------------------------------------
        // Text property custom getter/setter
        //---------------------------------------
        fn text(&self) -> String {
            self.pango_layout.get().unwrap().text().to_string()
        }

        fn set_text(&self, text: &str) {
            let obj = self.obj();

            // Create link/comment lists
            let mut link_list: Vec<TextTag> = vec![];
            let mut comment_list: Vec<TextTag> = vec![];

            // Set pango layout text and store links in link map
            let layout = self.pango_layout.get().unwrap();

            layout.set_text(text);

            match obj.ptype() {
                PropType::Link => {
                    link_list.push(TextTag::new(text, 0, text.len().to_u32().unwrap()));
                },
                PropType::Packager => {
                    static EXPR: OnceLock<Regex> = OnceLock::new();

                    let expr = EXPR.get_or_init(|| {
                        Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,6}")
                            .expect("Regex error")
                    });

                    if let Some(m) = expr.find(text) {
                        link_list.push(TextTag::new(
                            &format!("mailto:{}", m.as_str()),
                            m.start().to_u32().unwrap(),
                            m.end().to_u32().unwrap()
                        ));
                    }
                },
                PropType::LinkList => {
                    if text.is_empty() {
                        layout.set_text("None");
                    } else {
                        static EXPR: OnceLock<FancyRegex> = OnceLock::new();

                        let expr = EXPR.get_or_init(|| {
                            FancyRegex::new(r"(?<=^|     )[a-zA-Z0-9@._+-]+(?=<|>|=|:|     |$)")
                                .expect("Regex error")
                        });

                        link_list.extend(expr.find_iter(text)
                            .flatten()
                            .map(|m| {
                                TextTag::new(
                                    &format!("pkg://{}", m.as_str()),
                                    m.start().to_u32().unwrap(),
                                    m.end().to_u32().unwrap()
                                )
                            })
                        );

                        comment_list.extend(text.match_indices(" [INSTALLED]")
                            .map(|(i, s)| {
                                TextTag::new(
                                    s,
                                    i.to_u32().unwrap(),
                                    (i.to_usize().unwrap() + s.len()).to_u32().unwrap()
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

            // Format pango layout text
            self.do_format();

            self.selection_start.set(None);
            self.selection_end.set(None);

            obj.set_expanded(false);

            self.draw_area.queue_resize();
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
    // New function
    //---------------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //---------------------------------------
    // Pango color helper function
    //---------------------------------------
    fn pango_color_from_style(&self, style: &str) -> (u16, u16, u16, u16) {
        let label = gtk::Label::builder()
            .css_name("texttag")
            .css_classes([style])
            .build();

        let color = label.color();

        ((color.red() * 65535.0) as u16, (color.green() * 65535.0) as u16, (color.blue() * 65535.0) as u16, (color.alpha() * 65535.0) as u16)
    }

    //---------------------------------------
    // Setup widget
    //---------------------------------------
    fn setup_widget(&self) {
        let imp = self.imp();

        // Reset selection
        imp.selection_start.set(None);
        imp.selection_end.set(None);

        // Initialize pango colors
        imp.link_fg_color.set(self.pango_color_from_style("link"));
        imp.comment_fg_color.set(self.pango_color_from_style("comment"));
        imp.sel_bg_color.set(self.pango_color_from_style("selection"));
        imp.sel_focus_bg_color.set(self.pango_color_from_style("selection-focus"));
    }

    //---------------------------------------
    // Setup layout
    //---------------------------------------
    fn setup_layout(&self) {
        let imp = self.imp();

        // Create pango layout
        let layout = imp.draw_area.create_pango_layout(None);
        layout.set_wrap(pango::WrapMode::Word);
        layout.set_line_spacing(1.15);

        imp.pango_layout.set(layout).unwrap();

        // Connect drawing area draw function
        imp.draw_area.set_draw_func(clone!(
            #[weak(rename_to = widget)] self,
            #[weak] imp,
            move |_, context, _, _| {
                let layout = imp.pango_layout.get().unwrap();

                // Check if text widget can expand
                let measure_layout = layout.copy();
                measure_layout.set_width((imp.draw_area.width() - EXPAND_MARGIN) * pango::SCALE);

                let can_expand = if widget.expanded() {
                    measure_layout.line_count() > widget.max_lines()
                } else {
                    measure_layout.is_ellipsized()
                };

                // Adjust pango layout width if text widget can expand
                if can_expand {
                    layout.set_width((imp.draw_area.width() - EXPAND_MARGIN) * pango::SCALE);
                } else {
                    layout.set_width(imp.draw_area.width() * pango::SCALE);
                }

                // Set can expand property if changed
                if widget.can_expand() != can_expand {
                    widget.set_can_expand(can_expand);
                }

                // Format pango layout text selection
                if let Some(attr_list) = layout.attributes()
                    .and_then(|list| list.filter(|attr| attr.type_() != pango::AttrType::Background && attr.type_() != pango::AttrType::BackgroundAlpha))
                {
                    if let (Some(start), Some(end)) = (imp.selection_start.get(), imp.selection_end.get())
                    {
                        if start != end {
                            imp.format_selection(&attr_list, start.min(end), start.max(end));
                        }
                    }

                    layout.set_attributes(Some(&attr_list));
                }

                // Show pango layout
                let text_color = widget.color();

                context.set_source_rgba(text_color.red() as f64, text_color.green() as f64, text_color.blue() as f64, text_color.alpha() as f64);
                context.move_to(0.0, 0.0);

                pangocairo::functions::show_layout(context, layout);

                // Draw link focus indicator
                let link_list = imp.link_list.borrow();

                let index = imp.focus_link_index.get();

                if widget.focused() {
                    if let Some(link) = index.and_then(|i| link_list.get(i)) {
                        let link_start = link.start.to_i32().unwrap();
                        let link_end = link.end.to_i32().unwrap();

                        let (start_n, start_x) = layout.index_to_line_x(link_start, false);
                        let (end_n, end_x) = layout.index_to_line_x(link_end, false);

                        if start_n == end_n {
                            // Link is all on one line
                            let start_char_rect = layout.index_to_pos(link_start);

                            let y = pango::units_to_double(start_char_rect.y() + start_char_rect.height()) - 1.0;

                            context.move_to(pango::units_to_double(start_x), y);
                            context.line_to(pango::units_to_double(end_x), y);
                        } else {
                            // Link is split across lines
                            let start_char_rect = layout.index_to_pos(link_start);
                            let end_char_rect = layout.index_to_pos(link_end);

                            let (_, start_line_rect) = layout.line_readonly(start_n).unwrap().extents();

                            let start_y = pango::units_to_double(start_char_rect.y() + start_char_rect.height()) - 1.0;

                            context.move_to(pango::units_to_double(start_x), start_y);
                            context.line_to(pango::units_to_double(start_line_rect.width()), start_y);

                            let end_y = pango::units_to_double(end_char_rect.y() + end_char_rect.height()) - 1.0;

                            context.move_to(0.0, end_y);
                            context.line_to(pango::units_to_double(end_x), end_y);
                        }

                        let (red, green, blue, alpha) = imp.link_fg_color.get();

                        context.set_source_rgba(red as f64 / 65535.0, green as f64 / 65535.0, blue as f64 / 65535.0, (alpha as f64)/2.0 / 65535.0);

                        context.set_line_width(2.0);
                        context.stroke().unwrap();
                    }
                }
            }
        ));
    }

    //---------------------------------------
    // Action helper functions
    //---------------------------------------
    fn selected_text(&self) -> Option<String> {
        let imp = self.imp();

        if let (Some(start), Some(end)) = (imp.selection_start.get(), imp.selection_end.get()) {
            self.text().get(start.min(end) as usize..start.max(end) as usize)
                .map(|s| s.to_string())
        } else {
            None
        }
    }

    //---------------------------------------
    // Setup actions
    //---------------------------------------
    fn setup_actions(&self) {
        let imp = self.imp();

        // Add selection actions
        let select_all_action = gio::ActionEntry::builder("select-all")
            .activate(clone!(
                #[weak(rename_to = widget)] self,
                #[weak] imp,
                move |_, _, _| {
                    imp.selection_start.set(Some(0));
                    imp.selection_end.set(widget.text().len().to_u32());

                    imp.draw_area.queue_draw();
                }
            ))
            .build();

        let select_none_action = gio::ActionEntry::builder("select-none")
            .activate(clone!(
                #[weak] imp,
                move |_, _, _| {
                    imp.selection_start.set(None);
                    imp.selection_end.set(None);

                    imp.draw_area.queue_draw();
                }
            ))
            .build();

        // Add copy action
        let copy_action = gio::ActionEntry::builder("copy")
            .activate(clone!(
                #[weak(rename_to = widget)] self,
                move |_, _, _| {
                    if let Some(text) = widget.selected_text() {
                        widget.clipboard().set_text(&text);
                    }
                }
            ))
            .build();

        // Add expand/contract actions
        let expand_action = gio::ActionEntry::builder("expand")
            .activate(clone!(
                #[weak(rename_to = widget)] self,
                move |_, _, _| {
                    if !widget.expanded() {
                        widget.set_expanded(true);
                    }
                }
            ))
            .build();

        let contract_action = gio::ActionEntry::builder("contract")
            .activate(clone!(
                #[weak(rename_to = widget)] self,
                move |_, _, _| {
                    if widget.expanded() {
                        widget.set_expanded(false);
                    }
                }
            ))
            .build();

        // Add link actions
        let prev_link_action = gio::ActionEntry::builder("previous-link")
            .activate(clone!(
                #[weak] imp,
                move |_, _, _| {
                    let link_list = imp.link_list.borrow();

                    if let Some(new_index) = imp.focus_link_index.get()
                        .and_then(|i| i.checked_sub(1))
                        .or_else(|| link_list.len().checked_sub(1))
                    {
                        imp.focus_link_index.set(Some(new_index));

                        imp.draw_area.queue_draw();
                    }
                }
            ))
            .build();

        let next_link_action = gio::ActionEntry::builder("next-link")
            .activate(clone!(
                #[weak] imp,
                move |_, _, _| {
                    let link_list = imp.link_list.borrow();

                    if let Some(new_index) = imp.focus_link_index.get()
                        .and_then(|i| i.checked_add(1))
                        .filter(|&i| i < link_list.len())
                        .or_else(|| if link_list.is_empty() { None } else { Some(0) })
                    {
                        imp.focus_link_index.set(Some(new_index));

                        imp.draw_area.queue_draw();
                    }
                }
            ))
            .build();

        let activate_link_action = gio::ActionEntry::builder("activate-link")
            .activate(clone!(
                #[weak(rename_to = widget)] self,
                #[weak] imp,
                move |_, _, _| {
                    let link_list = imp.link_list.borrow();

                    if let Some(focus_link) = imp.focus_link_index.get()
                        .and_then(|i| link_list.get(i))
                    {
                        let link_url = focus_link.text();

                        // Need to drop to avoid panic
                        drop(link_list);

                        widget.handle_link(&link_url);
                    }
                }
            ))
            .build();

        // Add actions to text action group
        let text_group = gio::SimpleActionGroup::new();

        self.insert_action_group("text", Some(&text_group));

        text_group.add_action_entries([select_all_action, select_none_action, copy_action, expand_action, contract_action, prev_link_action, next_link_action, activate_link_action]);
    }

    //---------------------------------------
    // Resize layout helper function
    //---------------------------------------
    fn resize_layout(&self) {
        let imp = self.imp();

        let layout = imp.pango_layout.get().unwrap();

        if self.expanded() {
            layout.set_height(-1);
            layout.set_ellipsize(pango::EllipsizeMode::None);
        } else {
            layout.set_height(-self.max_lines());
            layout.set_ellipsize(pango::EllipsizeMode::End);
        }

        imp.draw_area.queue_resize();
    }

    //---------------------------------------
    // Update pango colors helper function
    //---------------------------------------
    fn update_pango_colors(&self) {
        let imp = self.imp();

        // Update pango color variables
        imp.link_fg_color.set(self.pango_color_from_style("link"));
        imp.comment_fg_color.set(self.pango_color_from_style("comment"));
        imp.sel_bg_color.set(self.pango_color_from_style("selection"));
        imp.sel_focus_bg_color.set(self.pango_color_from_style("selection-focus"));

        // Format pango layout text
        imp.do_format();

        // Redraw widget
        imp.draw_area.queue_draw();
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        // Expanded property notify signal
        self.connect_expanded_notify(|widget| {
            widget.resize_layout();
        });

        // Max lines property notify signal
        self.connect_max_lines_notify(|widget| {
            if !widget.expanded() {
                widget.resize_layout();
            }
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
    // Controller helper functions
    //---------------------------------------
    fn _inside_index_at_xy(&self, x: f64, y: f64) -> (bool, i32) {
        let layout = self.imp().pango_layout.get().unwrap();

        let (inside, mut index, trailing) = layout.xy_to_index(pango::units_from_double(x), pango::units_from_double(y));

        if trailing > 0 {
            index += 1;
        }

        (inside, index)
    }

    fn index_at_xy(&self, x: f64, y: f64) -> i32 {
        let (_, index) = self._inside_index_at_xy(x, y);

        index
    }

    fn link_url_at_xy(&self, x: f64, y: f64) -> Option<String> {
        let (inside, index) = self._inside_index_at_xy(x, y);

        if inside && index >= 0 {
            return self.imp().link_list.borrow().iter()
                .find(|link| link.start <= index as u32 && link.end > index as u32)
                .map(|link| link.text())
        }

        None
    }

    fn set_motion_cursor(&self, x: f64, y: f64) {
        let imp = self.imp();

        if !imp.is_selecting.get() {
            // Get cursor
            let cursor = if self.link_url_at_xy(x, y).is_some() {
                "pointer"
            } else {
                "text"
            };

            // Update cursor if necessary
            if cursor != *imp.cursor.borrow() {
                imp.draw_area.set_cursor_from_name(Some(cursor));

                imp.cursor.replace(cursor.to_string());
            }
        }
    }

    fn handle_link(&self, link_url: &str) {
        if let Ok(url) = Url::parse(link_url) {
            let url_scheme = url.scheme();

            if url_scheme == "pkg" {
                let pkg_name = url.domain().unwrap_or_default();

                self.emit_by_name::<()>("package-link", &[&pkg_name]);
            } else if let Some(handler) = gio::AppInfo::default_for_uri_scheme(url_scheme) {
                let _ = handler.launch_uris(&[link_url], None::<&gio::AppLaunchContext>);
            }
        }
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Add mouse motion controller
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

        // Add mouse drag controller
        let drag_controller = gtk::GestureDrag::new();

        drag_controller.connect_drag_begin(clone!(
            #[weak(rename_to = widget)] self,
            #[weak] imp,
            move |_, x, y| {
                if widget.link_url_at_xy(x, y).is_none() {
                    if !imp.is_clicked.get() {
                        let index = widget.index_at_xy(x, y);

                        imp.selection_start.set(index.to_u32());
                        imp.selection_end.set(None);
                    }

                    imp.is_selecting.set(true);
                }
            }
        ));

        drag_controller.connect_drag_update(clone!(
            #[weak(rename_to = widget)] self,
            #[weak] imp,
            move |controller, x, y| {
                if let Some((start_x, start_y)) = controller.start_point() {
                    let index = widget.index_at_xy(start_x + x, start_y + y);

                    imp.selection_end.set(index.to_u32());

                    imp.draw_area.queue_draw();
                }
            }
        ));

        drag_controller.connect_drag_end(clone!(
            #[weak] imp,
            move |_, _, _| {
                // Redraw if necessary to hide selection
                let start = imp.selection_start.get();
                let end = imp.selection_end.get();

                if end.is_none() || start == end {
                    imp.selection_start.set(None);
                    imp.selection_end.set(None);

                    imp.draw_area.queue_draw();
                }

                imp.is_selecting.set(false);
            }
        ));

        imp.draw_area.add_controller(drag_controller);

        // Add mouse click gesture controller
        let click_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();

        click_gesture.connect_pressed(clone!(
            #[weak(rename_to = widget)] self,
            #[weak] imp,
            move |_, n, x, y| {
                let link_url = widget.link_url_at_xy(x, y);

                if link_url.is_none() {
                    if n == 2 {
                        // Double click: select word under cursor and redraw widget
                        imp.is_clicked.set(true);

                        let index = widget.index_at_xy(x, y).to_usize().unwrap();

                        let text = widget.text();

                        let start = text.get(..index)
                            .and_then(|s| {
                                s.bytes()
                                    .rposition(|ch: u8| ch.is_ascii_whitespace() || ch.is_ascii_punctuation())
                                    .and_then(|start| start.checked_add(1))
                            })
                            .unwrap_or(0);

                        let end = text.get(index..)
                            .and_then(|s| {
                                s.bytes()
                                    .position(|ch: u8| ch.is_ascii_whitespace() || ch.is_ascii_punctuation())
                                    .and_then(|end| end.checked_add(index))
                            })
                            .unwrap_or(text.len());

                        imp.selection_start.set(start.to_u32());
                        imp.selection_end.set(end.to_u32());

                        imp.draw_area.queue_draw();
                    } else if n == 3 {
                        // Triple click: select all text and redraw widget
                        imp.is_clicked.set(true);

                        imp.selection_start.set(Some(0));
                        imp.selection_end.set(widget.text().len().to_u32());

                        imp.draw_area.queue_draw();
                    }
                }

                imp.pressed_link_url.replace(link_url);
            }
        ));

        click_gesture.connect_released(clone!(
            #[weak(rename_to = widget)] self,
            #[weak] imp,
            move |_, _, x, y| {
                imp.is_clicked.set(false);

                // Launch link if any
                if let Some(link_url) = imp.pressed_link_url.take()
                    .filter(|pressed| { widget.link_url_at_xy(x, y).as_ref() == Some(pressed)})
                {
                    widget.handle_link(&link_url);
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
        let rect = gdk::Rectangle::new(x.to_i32().unwrap(), y.to_i32().unwrap(), 0, 0);

        imp.popover_menu.set_pointing_to(Some(&rect));
        imp.popover_menu.popup();
    }
}

impl Default for TextWidget {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        Self::new()
    }
}
