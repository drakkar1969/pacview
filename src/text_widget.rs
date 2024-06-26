use std::cell::{Cell, RefCell, OnceCell};
use std::sync::OnceLock;

use gtk::{gio, glib, gdk, pango};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use glib::subclass::Signal;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use url::Url;

//------------------------------------------------------------------------------
// CONST: Layout padding
//------------------------------------------------------------------------------
const PADDING: i32 = 4;

//------------------------------------------------------------------------------
// GLOBAL: Pango color from CSS function
//------------------------------------------------------------------------------
fn pango_color_from_css(css: &str) -> (u16, u16, u16, u16) {
    let label = gtk::Label::builder()
        .css_classes(["css-label"])
        .build();

    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_string(&format!("label.css-label {{ color: {css}; }}"));

    gtk::style_context_add_provider_for_display(&label.display(), &css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

    let color = label.color();

    gtk::style_context_remove_provider_for_display(&label.display(), &css_provider);

    ((color.red() * 65535.0) as u16, (color.green() * 65535.0) as u16, (color.blue() * 65535.0) as u16, (color.alpha() * 65535.0) as u16)
}

//------------------------------------------------------------------------------
// GLOBAL: Pango color Variables
//------------------------------------------------------------------------------
const LINK_CSS: &str = "@accent_color";
const COMMENT_CSS: &str = "@success_color";
const SELECTED_CSS: &str = "alpha(@view_fg_color, 0.1)";
const SELECTED_CSS_FOCUS: &str = "alpha(@accent_bg_color, 0.3)";

thread_local! {
    static LINK_RGBA: Cell<(u16, u16, u16, u16)> = Cell::new(pango_color_from_css(LINK_CSS));

    static COMMENT_RGBA: Cell<(u16, u16, u16, u16)> = Cell::new(pango_color_from_css(COMMENT_CSS));

    static SELECTED_RGBA: Cell<(u16, u16, u16, u16)> = Cell::new(pango_color_from_css(SELECTED_CSS));

    static SELECTED_RGBA_FOCUS: Cell<(u16, u16, u16, u16)> = Cell::new(pango_color_from_css(SELECTED_CSS_FOCUS));
}

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
#[derive(Debug, Eq, PartialEq, Clone)]
struct Marker {
    text: String,
    start: usize,
    end: usize,
}

impl Marker {
    fn text(&self) -> String {
        self.text.to_owned()
    }
}

//------------------------------------------------------------------------------
// MODULE: TextWidget
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
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

        pub(super) pango_layout: OnceCell<pango::Layout>,

        pub(super) link_list: RefCell<Vec<Marker>>,
        pub(super) comment_list: RefCell<Vec<Marker>>,

        pub(super) cursor: RefCell<String>,
        pub(super) pressed_link_url: RefCell<Option<String>>,

        pub(super) is_selecting: Cell<bool>,
        pub(super) is_clicked: Cell<bool>,
        pub(super) selection_start: Cell<i32>,
        pub(super) selection_end: Cell<i32>,

        pub(super) focus_link_index: Cell<Option<usize>>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for TextWidget {
        const NAME: &'static str = "TextWidget";
        type Type = super::TextWidget;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("text-widget");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TextWidget {
        //-----------------------------------
        // Custom signals
        //-----------------------------------
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

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_layout();
            obj.setup_actions();
            obj.setup_shortcuts();
            obj.setup_signals();
            obj.setup_controllers();
        }

        //-----------------------------------
        // Dispose function
        //-----------------------------------
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
                if for_size != -1 {
                    measure_layout.set_width(for_size * pango::SCALE);
                }

                (measure_layout.pixel_size().1 + 4 * PADDING, measure_layout.pixel_size().1 + 4 * PADDING, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let layout = self.pango_layout.get().unwrap();

            layout.set_width(width * pango::SCALE);

            self.draw_area.allocate(width, height, baseline, None);
        }
    }

    impl TextWidget {
        //-----------------------------------
        // Layout format helper functions
        //-----------------------------------
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

            let (red, green, blue, alpha) = LINK_RGBA.get();

            for link in link_list {
                let start = link.start as u32;
                let end = link.end as u32;

                let mut attr = pango::AttrColor::new_foreground(red, green, blue);
                attr.set_start_index(start);
                attr.set_end_index(end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_foreground_alpha(alpha);
                attr.set_start_index(start);
                attr.set_end_index(end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_underline(pango::Underline::Single);
                attr.set_start_index(start);
                attr.set_end_index(end);

                attr_list.insert(attr);
            }
        }

        fn format_comments(&self, attr_list: &pango::AttrList) {
            let comment_list = &*self.comment_list.borrow();

            let (red, green, blue, alpha) = COMMENT_RGBA.get();

            for comment in comment_list {
                let start = comment.start as u32;
                let end = comment.end as u32;

                let mut attr = pango::AttrColor::new_foreground(red, green, blue);
                attr.set_start_index(start);
                attr.set_end_index(end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_foreground_alpha(alpha);
                attr.set_start_index(start);
                attr.set_end_index(end);

                attr_list.insert(attr);

                let mut attr = pango::AttrInt::new_weight(pango::Weight::Medium);
                attr.set_start_index(start);
                attr.set_end_index(end);

                attr_list.insert(attr);

                let mut attr = pango::AttrFloat::new_scale(0.9);
                attr.set_start_index(start);
                attr.set_end_index(end);

                attr_list.insert(attr);
            }
        }

        pub(super) fn format_selection(&self, attr_list: &pango::AttrList, start: u32, end: u32) {
            let (red, green, blue, alpha) = if self.obj().has_focus() {
                SELECTED_RGBA_FOCUS.get()
            } else {
                SELECTED_RGBA.get()
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

        //-----------------------------------
        // Text property custom getter/setter
        //-----------------------------------
        fn text(&self) -> String {
            self.pango_layout.get().unwrap().text().to_string()
        }

        fn set_text(&self, text: &str) {
            let obj = self.obj();

            // Create link/comment lists
            let mut link_list: Vec<Marker> = vec![];
            let mut comment_list: Vec<Marker> = vec![];

            // Set pango layout text and store links in link map
            let layout = self.pango_layout.get().unwrap();

            layout.set_text(text);

            match obj.ptype() {
                PropType::Link => {
                    link_list.push(Marker {
                        text: text.to_string(),
                        start: 0,
                        end: text.len()
                    });
                },
                PropType::Packager => {
                    static EXPR: OnceLock<Regex> = OnceLock::new();

                    let expr = EXPR.get_or_init(|| {
                        Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,6}")
                            .expect("Regex error")
                    });

                    if let Some(m) = expr.find(text) {
                        link_list.push(Marker {
                            text: format!("mailto:{}", m.as_str()),
                            start: m.start(),
                            end: m.end()
                        });
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

                        for m in expr.find_iter(text).flatten() {
                            link_list.push(Marker {
                                text: format!("pkg://{}", m.as_str()),
                                start: m.start(),
                                end: m.end()
                            });
                        }

                        let indices = text.match_indices(" [INSTALLED]");

                        for (i, s) in indices {
                            comment_list.push(Marker {
                                text: s.to_string(),
                                start: i,
                                end: i + s.len()
                            });
                        }
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

            self.selection_start.set(-1);
            self.selection_end.set(-1);

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
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup layout
    //-----------------------------------
    fn setup_layout(&self) {
        let imp = self.imp();

        // Create pango layout
        let layout = imp.draw_area.create_pango_layout(None);
        layout.set_wrap(pango::WrapMode::Word);
        layout.set_line_spacing(1.15);

        imp.pango_layout.set(layout).unwrap();

        // Set drawing margins
        imp.draw_area.set_margin_top(PADDING);
        imp.draw_area.set_margin_bottom(PADDING);

        // Connect drawing area draw function
        imp.draw_area.set_draw_func(clone!(@weak self as widget, @weak imp => move |_, context, _, _| {
            let layout = imp.pango_layout.get().unwrap();

            // Format pango layout text selection
            if let Some(attr_list) = layout.attributes()
                .and_then(|list| list.filter(|attr| attr.type_() != pango::AttrType::Background && attr.type_() != pango::AttrType::BackgroundAlpha))
            {
                let start = imp.selection_start.get();
                let end = imp.selection_end.get();

                if start != -1 && end != -1 && start != end {
                    imp.format_selection(&attr_list, start.min(end) as u32, start.max(end) as u32);
                }

                layout.set_attributes(Some(&attr_list));
            }

            // Show pango layout
            let text_color = widget.color();

            context.set_source_rgba(text_color.red() as f64, text_color.green() as f64, text_color.blue() as f64, text_color.alpha() as f64);
            context.move_to(0.0, PADDING as f64);

            pangocairo::functions::show_layout(context, layout);

            // Draw link focus indicator
            let link_list = imp.link_list.borrow();

            let index = imp.focus_link_index.get();

            if widget.has_focus() {
                if let Some(link) = index.and_then(|i| link_list.get(i)) {
                    let (start_n, start_x) = layout.index_to_line_x(link.start as i32, false);
                    let (end_n, end_x) = layout.index_to_line_x(link.end as i32, false);

                    if start_n == end_n {
                        // Link is all on one line
                        let start_char_rect = layout.index_to_pos(link.start as i32);

                        let y = pango::units_to_double(start_char_rect.y() + start_char_rect.height()) + PADDING as f64;

                        context.move_to(pango::units_to_double(start_x), y);
                        context.line_to(pango::units_to_double(end_x), y);
                    } else {
                        // Link is split across lines
                        let start_char_rect = layout.index_to_pos(link.start as i32);
                        let end_char_rect = layout.index_to_pos(link.end as i32);

                        let (_, start_line_rect) = layout.line_readonly(start_n).unwrap().extents();

                        let start_y = pango::units_to_double(start_char_rect.y() + start_char_rect.height()) + PADDING as f64;

                        context.move_to(pango::units_to_double(start_x), start_y);
                        context.line_to(pango::units_to_double(start_line_rect.width()), start_y);

                        let end_y = pango::units_to_double(end_char_rect.y() + end_char_rect.height()) + PADDING as f64;

                        context.move_to(0.0, end_y);
                        context.line_to(pango::units_to_double(end_x), end_y);
                    }

                    let (red, green, blue, alpha) = LINK_RGBA.get();

                    context.set_source_rgba(red as f64 / 65535.0, green as f64 / 65535.0, blue as f64 / 65535.0, alpha as f64 / 65535.0);

                    context.set_line_width(2.0);
                    context.stroke().unwrap();
                }
            }
        }));
    }

    //-----------------------------------
    // Action helper functions
    //-----------------------------------
    fn selected_text(&self) -> Option<String> {
        let imp = self.imp();

        let start = imp.selection_start.get() as usize;
        let end = imp.selection_end.get() as usize;

        self.text().get(start.min(end)..start.max(end))
            .map(|s| s.to_string())
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        let imp = self.imp();

        // Add selection actions
        let select_all_action = gio::ActionEntry::builder("select-all")
            .activate(clone!(@weak self as widget, @weak imp => move |_, _, _| {
                imp.selection_start.set(0);
                imp.selection_end.set(widget.text().len() as i32);

                imp.draw_area.queue_draw();
            }))
            .build();

        let select_none_action = gio::ActionEntry::builder("select-none")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.selection_start.set(-1);
                imp.selection_end.set(-1);

                imp.draw_area.queue_draw();
            }))
            .build();

        // Add copy action
        let copy_action = gio::ActionEntry::builder("copy")
            .activate(clone!(@weak self as widget => move |_, _, _| {
                if let Some(text) = widget.selected_text() {
                    widget.clipboard().set_text(&text);
                }
            }))
            .build();

        // Add actions to text action group
        let text_group = gio::SimpleActionGroup::new();

        self.insert_action_group("text", Some(&text_group));

        text_group.add_action_entries([select_all_action, select_none_action, copy_action]);
    }

    //-----------------------------------
    // Setup shortcuts
    //-----------------------------------
    fn setup_shortcuts(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();

        // Add selection shortcuts
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>A"),
            Some(gtk::NamedAction::new("text.select-all"))
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>A"),
            Some(gtk::NamedAction::new("text.select-none"))
        ));

        // Add copy shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>C"),
            Some(gtk::NamedAction::new("text.copy"))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Has focus property notify signal
        self.connect_has_focus_notify(|widget| {
            let imp = widget.imp();

            if !widget.has_focus() {
                if !imp.link_list.borrow().is_empty() {
                    imp.focus_link_index.set(Some(0));
                } else {
                    imp.focus_link_index.set(None);
                }
            }

            imp.draw_area.queue_draw();
        });

        // Color scheme changed signal
        let style_manager = adw::StyleManager::for_display(&self.display());

        style_manager.connect_dark_notify(clone!(@weak imp => move |_| {
            // Update pango color variables
            LINK_RGBA.set(pango_color_from_css(LINK_CSS));
            COMMENT_RGBA.set(pango_color_from_css(COMMENT_CSS));
            SELECTED_RGBA.set(pango_color_from_css(SELECTED_CSS));
            SELECTED_RGBA_FOCUS.set(pango_color_from_css(SELECTED_CSS_FOCUS));

            // Format pango layout text
            imp.do_format();
        }));
    }

    //-----------------------------------
    // Controller helper functions
    //-----------------------------------
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

        if inside {
            return self.imp().link_list.borrow().iter()
                .find(|link| link.start <= index as usize && link.end > index as usize)
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
                let _ = handler.launch_uris(&[&link_url], None::<&gio::AppLaunchContext>);
            }
        }
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Add mouse motion controller
        let motion_controller = gtk::EventControllerMotion::new();

        motion_controller.connect_enter(clone!(@weak self as widget => move |_, x, y| {
            widget.set_motion_cursor(x, y);
        }));

        motion_controller.connect_motion(clone!(@weak self as widget => move |_, x, y| {
            widget.set_motion_cursor(x, y);
        }));

        imp.draw_area.add_controller(motion_controller);

        // Add mouse drag gesture controller
        let drag_controller = gtk::GestureDrag::new();

        drag_controller.connect_drag_begin(clone!(@weak self as widget, @weak imp => move |_, x, y| {
            if widget.link_url_at_xy(x, y).is_none() {
                if !imp.is_clicked.get() {
                    let index = widget.index_at_xy(x, y);

                    imp.selection_start.set(index);
                    imp.selection_end.set(-1);
                }

                imp.is_selecting.set(true);

                widget.grab_focus();
            }
        }));

        drag_controller.connect_drag_update(clone!(@weak self as widget, @weak imp => move |controller, x, y| {
            if let Some((start_x, start_y)) = controller.start_point() {
                let index = widget.index_at_xy(start_x + x, start_y + y);

                imp.selection_end.set(index);

                imp.draw_area.queue_draw();
            }
        }));

        drag_controller.connect_drag_end(clone!(@weak imp => move |_, _, _| {
            // Redraw if necessary to hide selection
            let start = imp.selection_start.get();
            let end = imp.selection_end.get();

            if end == -1 || start == end {
                imp.selection_start.set(-1);
                imp.selection_end.set(-1);

                imp.draw_area.queue_draw();
            }

            imp.is_selecting.set(false);
        }));

        imp.draw_area.add_controller(drag_controller);

        // Add mouse click gesture controller
        let click_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();

        click_gesture.connect_pressed(clone!(@weak self as widget, @weak imp => move |_, n, x, y| {
            let link_url = widget.link_url_at_xy(x, y);

            if link_url.is_none() {
                if n == 2 {
                    // Double click: select word under cursor and redraw widget
                    imp.is_clicked.set(true);

                    let index = widget.index_at_xy(x, y) as usize;

                    let text = widget.text();

                    let start = text.get(..index)
                        .and_then(|s| {
                            s.bytes()
                                .rposition(|ch: u8| ch.is_ascii_whitespace() || ch.is_ascii_punctuation())
                                .map(|start| start + 1)
                        })
                        .unwrap_or(0);

                    let end = text.get(index..)
                        .and_then(|s| {
                            s.bytes()
                                .position(|ch: u8| ch.is_ascii_whitespace() || ch.is_ascii_punctuation())
                                .map(|end| end + index)
                        })
                        .unwrap_or(text.len());

                    imp.selection_start.set(start as i32);
                    imp.selection_end.set(end as i32);

                    imp.draw_area.queue_draw();
                } else if n == 3 {
                    // Triple click: select all text and redraw widget
                    imp.is_clicked.set(true);

                    imp.selection_start.set(0);
                    imp.selection_end.set(widget.text().len() as i32);

                    imp.draw_area.queue_draw();
                }
            }

            imp.pressed_link_url.replace(link_url);
        }));

        click_gesture.connect_released(clone!(@weak self as widget, @weak imp => move |_, _, x, y| {
            imp.is_clicked.set(false);

            // Launch link if any
            if let Some(link_url) = imp.pressed_link_url.take()
                .and_then(|pressed| widget.link_url_at_xy(x, y).filter(|link| link == &pressed))
            {
                widget.handle_link(&link_url);
            }
        }));

        imp.draw_area.add_controller(click_gesture);

        // Add popup menu controller
        let popup_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();

        popup_gesture.connect_pressed(clone!(@weak self as widget, @weak imp => move |_, _, x, y| {
            // Enable/disable copy action
            widget.action_set_enabled("text.copy", widget.selected_text().is_some());

            // Show popover menu
            let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

            imp.popover_menu.set_pointing_to(Some(&rect));
            imp.popover_menu.popup();
        }));

        imp.draw_area.add_controller(popup_gesture);

        // Add key press controller
        let key_controller = gtk::EventControllerKey::new();

        key_controller.connect_key_pressed(clone!(@weak self as widget, @weak imp => @default-return glib::Propagation::Proceed, move |_, key, _, state| {
            if state == gdk::ModifierType::empty() && (key == gdk::Key::Left || key == gdk::Key::Right || key == gdk::Key::Return || key == gdk::Key::KP_Enter) {
                let link_list = imp.link_list.borrow();

                let index = imp.focus_link_index.get();

                if key == gdk::Key::Left {
                    let new_index = index
                        .and_then(|i| i.checked_sub(1))
                        .or_else(|| link_list.len().checked_sub(1));

                    if new_index.is_some()
                    {
                        imp.focus_link_index.set(new_index);

                        imp.draw_area.queue_draw();
                    }
                }

                if key == gdk::Key::Right {
                    let new_index = index
                        .and_then(|i| i.checked_add(1))
                        .filter(|&i| i < link_list.len())
                        .or_else(|| if link_list.is_empty() { None } else { Some(0) });

                    if new_index.is_some()
                    {
                        imp.focus_link_index.set(new_index);

                        imp.draw_area.queue_draw();
                    }
                }

                if key == gdk::Key::Return || key == gdk::Key::KP_Enter {
                    if let Some(focus_link) = index.and_then(|i| link_list.get(i)) {
                        let link_url = focus_link.text();

                        // Need to drop to avoid panic
                        drop(link_list);

                        widget.handle_link(&link_url);
                    }
                }

                return glib::Propagation::Stop
            }

            glib::Propagation::Proceed
        }));

        self.add_controller(key_controller);
    }
}

impl Default for TextWidget {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
