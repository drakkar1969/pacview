use std::cell::{Cell, RefCell, OnceCell};
use std::sync::OnceLock;

use gtk::{gio, glib, gdk, pango};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use glib::subclass::Signal;

use fancy_regex::Regex;
use url::Url;

//------------------------------------------------------------------------------
// GLOBAL: Color Variables
//------------------------------------------------------------------------------
thread_local! {
    static LINK_RGBA: Cell<gdk::RGBA> = Cell::new({
        let link_btn = gtk::LinkButton::new("www.gtk.org");

        link_btn.color()
    });

    static SELECTED_RGBA: Cell<gdk::RGBA> = Cell::new({
        let label = gtk::Label::new(None);
        label.add_css_class("css-label");

        let style_manager = adw::StyleManager::default();

        let alpha = if style_manager.is_dark() { 0.7 } else { 0.3 };

        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_string(&format!("label.css-label {{ color: alpha(@accent_bg_color, {alpha}); }}"));

        gtk::style_context_add_provider_for_display(&label.display(), &css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

        let selected_rgba = label.color();

        gtk::style_context_remove_provider_for_display(&label.display(), &css_provider);

        selected_rgba
    });
}

//------------------------------------------------------------------------------
// ENUM: PropType
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PropType")]
pub enum PropType {
    #[default]
    Text = 0,
    Title = 1,
    Link = 2,
    Packager = 3,
    LinkList = 4,
}

//------------------------------------------------------------------------------
// STRUCT: Link
//------------------------------------------------------------------------------
pub struct Link {
    url: String,
    start: usize,
    end: usize,
}

impl Link {
    pub fn url(&self) -> String {
        self.url.to_owned()
    }
}

//------------------------------------------------------------------------------
// MODULE: TextLayout
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::TextLayout)]
    #[template(resource = "/com/github/PacView/ui/text_layout.ui")]
    pub struct TextLayout {
        #[template_child]
        pub draw_area: TemplateChild<gtk::DrawingArea>,
        #[template_child]
        pub popover_menu: TemplateChild<gtk::PopoverMenu>,

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(get = Self::text, set = Self::set_text)]
        _text: RefCell<String>,

        pub pango_layout: OnceCell<pango::Layout>,

        pub link_list: RefCell<Vec<Link>>,

        pub cursor: RefCell<String>,
        pub pressed_link: RefCell<Option<String>>,

        pub is_selecting: Cell<bool>,
        pub selection_start: Cell<i32>,
        pub selection_end: Cell<i32>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for TextLayout {
        const NAME: &'static str = "TextLayout";
        type Type = super::TextLayout;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("text-layout");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TextLayout {
        //-----------------------------------
        // Custom signals
        //-----------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("link-activated")
                        .param_types([String::static_type()])
                        .return_type::<bool>()
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
            obj.setup_controllers();
            obj.setup_signals();
        }

        //-----------------------------------
        // Dispose function
        //-----------------------------------
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for TextLayout {
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

                (measure_layout.pixel_size().1, measure_layout.pixel_size().1, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let layout = self.pango_layout.get().unwrap();

            layout.set_width(width * pango::SCALE);

            self.draw_area.allocate(width, height, baseline, None);
        }
    }

    impl TextLayout {
        //-----------------------------------
        // Layout format helper functions
        //-----------------------------------
        fn rgba_to_pango_rgb(&self, color: gdk::RGBA, bg_color: gdk::RGBA) -> (u16, u16, u16) {
            // Fake transparency (pango bug)
            let red = ((1.0 - color.alpha()) * bg_color.red()) + (color.red() * color.alpha());
            let green = ((1.0 - color.alpha()) * bg_color.green()) + (color.green() * color.alpha());
            let blue = ((1.0 - color.alpha()) * bg_color.blue()) + (color.blue() * color.alpha());

            ((red * 65535.0) as u16, (green * 65535.0) as u16, (blue * 65535.0) as u16)
        }

        fn format_text(&self, attr_list: &pango::AttrList, weight: pango::Weight) {
            let obj = self.obj();

            let (red, green, blue) = self.rgba_to_pango_rgb(obj.color(), obj.parent().unwrap().color());

            let mut attr = pango::AttrColor::new_foreground(red, green, blue);
            attr.set_start_index(pango::ATTR_INDEX_FROM_TEXT_BEGINNING);
            attr.set_end_index(pango::ATTR_INDEX_TO_TEXT_END);

            attr_list.insert(attr);

            let mut attr = pango::AttrInt::new_weight(weight);
            attr.set_start_index(pango::ATTR_INDEX_FROM_TEXT_BEGINNING);
            attr.set_end_index(pango::ATTR_INDEX_TO_TEXT_END);

            attr_list.insert(attr);
        }

        fn format_link(&self, attr_list: &pango::AttrList, start: u32, end: u32, color: (u16, u16, u16)) {
            let (red, green, blue) = color;

            let mut attr = pango::AttrColor::new_foreground(red, green, blue);
            attr.set_start_index(start);
            attr.set_end_index(end);

            attr_list.insert(attr);

            let mut attr = pango::AttrInt::new_underline(pango::Underline::Single);
            attr.set_start_index(start);
            attr.set_end_index(end);

            attr_list.insert(attr);
        }

        pub fn format_selection(&self, attr_list: &pango::AttrList, start: u32, end: u32) {
            SELECTED_RGBA.with(|selected_rgba| {
                let obj = self.obj();

                let (red, green, blue) = self.rgba_to_pango_rgb(selected_rgba.get(), obj.parent().unwrap().color());

                let mut attr = pango::AttrColor::new_background(red, green, blue);
                attr.set_start_index(start);
                attr.set_end_index(end);

                attr_list.insert(attr);
            });
        }

        pub fn do_format(&self, link_list: &[Link]) {
            let obj = self.obj();

            let layout = self.pango_layout.get().unwrap();

            let attr_list = pango::AttrList::new();

            // Format text
            if obj.ptype() == PropType::Title {
                self.format_text(&attr_list, pango::Weight::Bold);
            } else {
                self.format_text(&attr_list, pango::Weight::Normal);
            }

            // Format links
            LINK_RGBA.with(|link_rgba| {
                let link_color = self.rgba_to_pango_rgb(link_rgba.get(), obj.parent().unwrap().color());

                for link in link_list {
                    self.format_link(&attr_list, link.start as u32, link.end as u32, link_color);
                }
            });

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

            // Clear link map
            let mut link_list = self.link_list.borrow_mut();

            link_list.clear();

            // Set pango layout text and store links in link map
            let layout = self.pango_layout.get().unwrap();

            layout.set_text(text);

            match obj.ptype() {
                PropType::Link => {
                    link_list.push(Link {
                        url: text.to_string(),
                        start: 0,
                        end: text.len()
                    });
                },
                PropType::Packager => {
                    static EXPR: OnceLock<Regex> = OnceLock::new();

                    let expr = EXPR.get_or_init(|| {
                        Regex::new("^(?:[^<]+?)<([^>]+?)>$").expect("Regex error")
                    });

                    if let Some(m) = expr.captures(text)
                        .ok()
                        .and_then(|caps_opt| caps_opt.and_then(|caps| caps.get(1)))
                    {
                        link_list.push(Link {
                            url: format!("mailto:{}", m.as_str()),
                            start: m.start(),
                            end: m.end()
                        });
                    }
                },
                PropType::LinkList => {
                    if text.is_empty() {
                        layout.set_text("None");
                    } else {
                        static EXPR: OnceLock<Regex> = OnceLock::new();

                        let expr = EXPR.get_or_init(|| {
                            Regex::new("(?:^|     )([a-zA-Z0-9@._+-]+)(?=<|>|=|:|     |$)").expect("Regex error")
                        });

                        for caps in expr.captures_iter(text).flatten() {
                            if let Some(m) = caps.get(1) {
                                link_list.push(Link {
                                    url: format!("pkg://{}", m.as_str()),
                                    start: m.start(),
                                    end: m.end()
                                });
                            }
                        }
                    }
                },
                _ => {}
            }

            // Format pango layout text
            self.do_format(&link_list);

            self.selection_start.set(-1);
            self.selection_end.set(-1);

            self.draw_area.queue_resize();
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: TextLayout
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct TextLayout(ObjectSubclass<imp::TextLayout>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl TextLayout {
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

        // Connect drawing area draw function
        imp.draw_area.set_draw_func(clone!(@weak imp => move |_, context, _, _| {
            let layout = imp.pango_layout.get().unwrap();

            // Format pango layout text selection
            if let Some(attr_list) = layout.attributes()
                .and_then(|list| list.filter(|attr| attr.type_() != pango::AttrType::Background))
            {
                let start = imp.selection_start.get();
                let end = imp.selection_end.get();

                if start != -1 && end != -1 && start != end {
                    imp.format_selection(&attr_list, start.min(end) as u32, start.max(end) as u32);
                }

                layout.set_attributes(Some(&attr_list));
            }

            // Show pango layout
            pangocairo::functions::show_layout(context, layout);
        }));
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        let imp = self.imp();

        // Add select all action
        let select_action = gio::ActionEntry::builder("select-all")
            .activate(clone!(@weak self as layout, @weak imp => move |_, _, _| {
                imp.selection_start.set(0);
                imp.selection_end.set(layout.text().len() as i32);

                imp.draw_area.queue_draw();
            }))
            .build();

        // Add copy action
        let copy_action = gio::ActionEntry::builder("copy")
            .activate(clone!(@weak self as layout, @weak imp => move |_, _, _| {
                let start = imp.selection_start.get() as usize;
                let end = imp.selection_end.get() as usize;

                if let Some(text) = layout.text().get(start.min(end)..start.max(end)) {
                    layout.clipboard().set_text(text);
                }
            }))
            .build();

        // Add actions to text action group
        let text_group = gio::SimpleActionGroup::new();

        self.insert_action_group("text", Some(&text_group));

        text_group.add_action_entries([select_action, copy_action]);
    }

    //-----------------------------------
    // Controller helper functions
    //-----------------------------------
    fn index_at_xy(&self, x: f64, y: f64) -> (bool, i32) {
        let layout = self.imp().pango_layout.get().unwrap();

        let (inside, mut index, trailing) = layout.xy_to_index(x as i32 * pango::SCALE, y as i32 * pango::SCALE);

        if trailing > 0 {
            index += 1;
        }

        (inside, index)
    }

    fn link_at_index(&self, inside: bool, index: i32) -> Option<String> {
        if inside {
            return self.imp().link_list.borrow().iter()
                .find(|link| link.start <= index as usize && link.end > index as usize)
                .map(|link| link.url())
        }

        None
    }

    fn link_at_xy(&self, x: f64, y: f64) -> Option<String> {
        let (inside, index) = self.index_at_xy(x, y);

        self.link_at_index(inside, index)
    }

    fn set_motion_cursor(&self, x: f64, y: f64) {
        let imp = self.imp();

        let (inside, index) = self.index_at_xy(x, y);

        let mut cursor = "text";

        // If text selection initiated, redraw to show selection
        if imp.is_selecting.get() {
            imp.selection_end.set(index);

            imp.draw_area.queue_draw();
        } else {
            // If no text selected, update cursor over links
            if self.link_at_index(inside, index).is_some() {
                cursor = "pointer";
            }
        }

        // Update cursor if necessary
        if cursor != *imp.cursor.borrow() {
            imp.draw_area.set_cursor_from_name(Some(cursor));

            imp.cursor.replace(cursor.to_string());
        }
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Add mouse move controller
        let motion_controller = gtk::EventControllerMotion::new();

        motion_controller.connect_enter(clone!(@weak self as layout => move |_, x, y| {
            layout.set_motion_cursor(x, y);
        }));

        motion_controller.connect_motion(clone!(@weak self as layout => move |_, x, y| {
            layout.set_motion_cursor(x, y);
        }));

        imp.draw_area.add_controller(motion_controller);

        // Add click gesture controller
        let click_gesture = gtk::GestureClick::new();
        click_gesture.set_button(gdk::BUTTON_PRIMARY);

        click_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

        click_gesture.connect_pressed(clone!(@weak self as layout, @weak imp => move |_, n, x, y| {
            let link = layout.link_at_xy(x, y);

            if link.is_none() {
                if n == 1 {
                    // Single click: start selection, widget is redrawn on mouse move
                    let (_, index) = layout.index_at_xy(x, y);

                    imp.selection_start.set(index);
                    imp.selection_end.set(-1);

                    imp.is_selecting.set(true);
                } else if n == 2 {
                    // Double click: select word under cursor and redraw widget
                    let (_, index) = layout.index_at_xy(x, y);

                    let text = layout.text();

                    let start = text.get(..index as usize)
                        .and_then(|s| {
                            s.bytes()
                                .rposition(|ch: u8| ch.is_ascii_whitespace() || ch.is_ascii_punctuation())
                                .map(|start| start + 1)
                        })
                        .unwrap_or(0);

                    let end = text.get(index as usize..)
                        .and_then(|s| {
                            s.bytes()
                                .position(|ch: u8| ch.is_ascii_whitespace() || ch.is_ascii_punctuation())
                                .map(|end| end + index as usize)
                        })
                        .unwrap_or(text.len());

                    imp.selection_start.set(start as i32);
                    imp.selection_end.set(end as i32);

                    imp.draw_area.queue_draw();
                } else if n == 3 {
                    // Triple click: select all text and redraw widget
                    imp.selection_start.set(0);
                    imp.selection_end.set(layout.text().len() as i32);

                    imp.draw_area.queue_draw();
                }
            }

            imp.pressed_link.replace(link);
        }));

        click_gesture.connect_released(clone!(@weak self as layout, @weak imp => move |_, _, x, y| {
            // Redraw if necessary to hide selection
            let start = imp.selection_start.get();
            let end = imp.selection_end.get();

            if imp.is_selecting.get() && (end == -1 || start == end) {
                imp.selection_start.set(-1);
                imp.selection_end.set(-1);

                imp.draw_area.queue_draw();
            }

            // End selection
            imp.is_selecting.set(false);

            // Launch link if any
            if let Some(link) = imp.pressed_link.take()
                .and_then(|pressed| layout.link_at_xy(x, y).filter(|link| link == &pressed))
            {
                if !layout.emit_by_name::<bool>("link-activated", &[&link]) {
                    if let Ok(url) = Url::parse(&link) {
                        if let Some(handler) = gio::AppInfo::default_for_uri_scheme(url.scheme()) {
                            let _ = handler.launch_uris(&[&link], None::<&gio::AppLaunchContext>);
                        }
                    }
                }
            }
        }));

        imp.draw_area.add_controller(click_gesture);

        // Add popup gesture controller
        let popup_gesture = gtk::GestureClick::new();
        popup_gesture.set_button(gdk::BUTTON_SECONDARY);

        popup_gesture.connect_pressed(clone!(@weak self as layout, @weak imp => move |_, _, x, y| {
            // Enable/disable copy action
            let start = imp.selection_start.get();
            let end = imp.selection_end.get();

            layout.action_set_enabled("text.copy", start != -1 && end != -1 && start != end);

            // Show popover menu
            let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

            imp.popover_menu.set_pointing_to(Some(&rect));
            imp.popover_menu.popup();
        }));

        imp.draw_area.add_controller(popup_gesture);
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Color scheme changed signal
        let style_manager = adw::StyleManager::default();

        style_manager.connect_dark_notify(clone!(@weak imp => move |style_manager| {
            // Update link color
            LINK_RGBA.with(|link_rgba| {
                let link_btn = gtk::LinkButton::new("www.gtk.org");

                let btn_style = adw::StyleManager::for_display(&link_btn.display());

                if style_manager.is_dark() {
                    btn_style.set_color_scheme(adw::ColorScheme::ForceDark);
                } else {
                    btn_style.set_color_scheme(adw::ColorScheme::ForceLight);
                }

                link_rgba.set(link_btn.color());
            });

            // Update selected background color
            SELECTED_RGBA.with(|selected_rgba| {
                let label = gtk::Label::new(None);
                label.add_css_class("css-label");

                let style_manager = adw::StyleManager::default();

                let alpha = if style_manager.is_dark() { 0.7 } else { 0.3 };

                let css_provider = gtk::CssProvider::new();
                css_provider.load_from_string(&format!("label.css-label {{ color: alpha(@accent_bg_color, {alpha}); }}"));

                gtk::style_context_add_provider_for_display(&label.display(), &css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

                selected_rgba.set(label.color());

                gtk::style_context_remove_provider_for_display(&label.display(), &css_provider);
            });

            // Format pango layout text
            let link_list = imp.link_list.borrow();

            imp.do_format(&link_list);
        }));
    }
}
