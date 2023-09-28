use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gtk::{gio, glib, gdk, pango};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use glib::once_cell::sync::{Lazy, OnceCell};
use glib::subclass::Signal;

use fancy_regex::Regex;
use lazy_static::lazy_static;
use url::Url;
use pangocairo;

//------------------------------------------------------------------------------
// ENUM: PropType
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PropType")]
pub enum PropType {
    Text = 0,
    Title = 1,
    Link = 2,
    Packager = 3,
    LinkList = 4,
}

impl Default for PropType {
    fn default() -> Self {
        PropType::Text
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
        #[property(set = Self::set_text)]
        _text: RefCell<String>,

        pub pango_layout: OnceCell<pango::Layout>,

        pub link_rgba: Cell<Option<gdk::RGBA>>,

        pub link_map: RefCell<HashMap<String, String>>,

        pub left_pressed: Cell<bool>,
        pub cursor: RefCell<String>,
        pub pressed_link: RefCell<Option<String>>,

        pub selection_start: Cell<i32>,
        pub selection_end: Cell<i32>,

        pub selection_bg_rgba: Cell<Option<gdk::RGBA>>,

        pub action_group: RefCell<gio::SimpleActionGroup>,
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
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("link-activated")
                        .param_types([String::static_type()])
                        .return_type::<bool>()
                        .build(),
                ]
            });
            SIGNALS.as_ref()
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
        // Text property custom setter
        //-----------------------------------
        fn set_text(&self, text: &str) {
            let layout = self.pango_layout.get().unwrap();

            layout.set_text(text);

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
    // Layout format helper functions
    //-----------------------------------
    fn rgba_to_pango_rgb(&self, color: gdk::RGBA, bg_color: gdk::RGBA) -> (u16, u16, u16) {
        let red = ((1.0 - color.alpha()) * bg_color.red()) + (color.red() * color.alpha());
        let green = ((1.0 - color.alpha()) * bg_color.green()) + (color.green() * color.alpha());
        let blue = ((1.0 - color.alpha()) * bg_color.blue()) + (color.blue() * color.alpha());

        ((red * 65535.0) as u16, (green * 65535.0) as u16, (blue * 65535.0) as u16)
    }

    fn format_text(&self, attr_list: &pango::AttrList, start: usize, end: usize, weight: pango::Weight) {
        let (red, green, blue) = self.rgba_to_pango_rgb(self.color(), self.parent().unwrap().color());

        let mut attr = pango::AttrColor::new_foreground(red, green, blue);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);

        let mut attr = pango::AttrInt::new_weight(weight);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);
    }

    fn format_link(&self, attr_list: &pango::AttrList, start: usize, end: usize) {
        let (red, green, blue) = self.rgba_to_pango_rgb(self.imp().link_rgba.get().unwrap(), self.parent().unwrap().color());

        let mut attr = pango::AttrColor::new_foreground(red, green, blue);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);

        let mut attr = pango::AttrInt::new_underline(pango::Underline::Single);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);
    }

    fn format_selection(&self, attr_list: &pango::AttrList, start: usize, end: usize) {
        let (red, green, blue) = self.rgba_to_pango_rgb(self.imp().selection_bg_rgba.get().unwrap(), self.parent().unwrap().color());

        let mut attr = pango::AttrColor::new_background(red, green, blue);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);
    }

    //-----------------------------------
    // Setup layout
    //-----------------------------------
    fn setup_layout(&self) {
        let imp = self.imp();

        // Get link/selected text colors
        let link_btn = gtk::LinkButton::new("www.gtk.org");

        imp.link_rgba.replace(Some(link_btn.color()));

        let label = gtk::Label::new(None);
        label.add_css_class("css-label");

        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_string(&format!("label.css-label {{ color: alpha(@accent_color, 0.3); }}"));

        gtk::style_context_add_provider_for_display(&label.display(), &css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

        imp.selection_bg_rgba.replace(Some(label.color()));

        gtk::style_context_remove_provider_for_display(&label.display(), &css_provider);

        // Clear selection markers
        imp.selection_start.set(-1);
        imp.selection_end.set(-1);

        // Create pango layout
        let layout = imp.draw_area.create_pango_layout(None);
        layout.set_wrap(pango::WrapMode::Word);
        layout.set_line_spacing(1.15);

        imp.pango_layout.set(layout).unwrap();

        // Connect drawing area draw function
        imp.draw_area.set_draw_func(clone!(@weak self as obj, @weak imp => move |_, context, _, _| {
            let layout = imp.pango_layout.get().unwrap();

            // Clear link map
            let mut link_map = imp.link_map.borrow_mut();

            link_map.clear();

            let attr_list = pango::AttrList::new();

            // Format pango layout text and store links in link map
            match obj.ptype() {
                PropType::Text => {
                    obj.format_text(&attr_list, 0, layout.text().len(), pango::Weight::Normal);
                },
                PropType::Title => {
                    obj.format_text(&attr_list, 0, layout.text().len(), pango::Weight::Bold);
                },
                PropType::Link => {
                    if layout.text().is_empty() {
                        layout.set_text("None");
                    }

                    if layout.text() == "None" {
                        obj.format_text(&attr_list, 0, layout.text().len(), pango::Weight::Normal);
                    } else {
                        link_map.insert(layout.text().to_string(), layout.text().to_string());

                        obj.format_link(&attr_list, 0, layout.text().len());
                    }
                },
                PropType::Packager => {
                    lazy_static! {
                        static ref EXPR: Regex = Regex::new("^(?:[^<]+?)<([^>]+?)>$").unwrap();
                    }

                    obj.format_text(&attr_list, 0, layout.text().len(), pango::Weight::Normal);

                    if let Some(m) = EXPR.captures(&layout.text()).ok().flatten().and_then(|caps| caps.get(1)) {
                        link_map.insert(m.as_str().to_string(), format!("mailto:{}", m.as_str()));

                        obj.format_link(&attr_list, m.start(), m.end());
                    }
                },
                PropType::LinkList => {
                    if layout.text().is_empty() {
                        layout.set_text("None");
                    }

                    if layout.text() == "None" {
                        obj.format_text(&attr_list, 0, layout.text().len(), pango::Weight::Normal);
                    } else {
                        lazy_static! {
                            static ref EXPR: Regex = Regex::new("(?:^|     )([a-zA-Z0-9@._+-]+)(?=<|>|=|:|     |$)").unwrap();
                        }

                        obj.format_text(&attr_list, 0, layout.text().len(), pango::Weight::Normal);

                        for caps in EXPR.captures_iter(&layout.text()).flatten() {
                            if let Some(m) = caps.get(1) {
                                link_map.insert(m.as_str().to_string(), format!("pkg://{}", m.as_str()));

                                obj.format_link(&attr_list, m.start(), m.end());
                            }
                        }

                    }
                },
            }

            // Format pango layout text selection
            let selection_start = imp.selection_start.get();
            let selection_end = imp.selection_end.get();

            if selection_start != -1 && selection_end != -1 && selection_start != selection_end {
                obj.format_selection(&attr_list, selection_start.min(selection_end) as usize, selection_start.max(selection_end) as usize);
            }

            // Show pango layout
            layout.set_attributes(Some(&attr_list));

            pangocairo::show_layout(&context, &layout);
        }));
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        let imp = self.imp();

        // Add select all action
        let select_action = gio::SimpleAction::new("select-all", None);
        select_action.connect_activate(clone!(@weak imp => move |_, _| {
            imp.selection_start.set(0);
            imp.selection_end.set(imp.pango_layout.get().unwrap().text().len() as i32);

            imp.draw_area.queue_draw();
        }));

        // Add copy action
        let copy_action = gio::SimpleAction::new("copy", None);
        copy_action.connect_activate(clone!(@weak self as obj, @weak imp => move |_, _| {
            let selection_start = imp.selection_start.get();
            let selection_end = imp.selection_end.get();

            if selection_start != -1 && selection_end != -1 && selection_start != selection_end {
                obj.clipboard().set_text(&imp.pango_layout.get().unwrap().text()[selection_start as usize..selection_end as usize]);
            }
        }));

        // Add actions to text action group
        let text_group = gio::SimpleActionGroup::new();

        self.insert_action_group("text", Some(&text_group));

        text_group.add_action(&select_action);
        text_group.add_action(&copy_action);

        imp.action_group.replace(text_group);
    }

    //-----------------------------------
    // Controller helper functions
    //-----------------------------------
    fn index_at_xy(&self, x: f64, y: f64) -> (bool, i32, i32) {
        let layout = self.imp().pango_layout.get().unwrap();

        layout.xy_to_index(x as i32 * pango::SCALE, y as i32 * pango::SCALE)
    }

    fn is_link_at_index(&self, inside: bool, index: i32) -> Option<pango::Attribute> {
        let layout = self.imp().pango_layout.get().unwrap();

        if inside {
            return layout.attributes().and_then(|attr_list| {
                attr_list.attributes().into_iter()
                    .find(|attr| attr.attr_class().type_() == pango::AttrType::Underline && attr.start_index() as i32 <= index && attr.end_index() as i32 > index)
            })
        }

        None
    }

    fn link_at_xy(&self, x: f64, y: f64) -> Option<String> {
        let imp = self.imp();

        let (inside, index, _) = self.index_at_xy(x, y);

        if let Some(attr) = self.is_link_at_index(inside, index) {
            let layout = imp.pango_layout.get().unwrap();

            let link_map = imp.link_map.borrow();

            if let Some(link) = link_map.get(&layout.text()[attr.start_index() as usize..attr.end_index() as usize])
            {
                return Some(link.to_string())
            }
        }

        None
    }

    fn set_cursor_motion(&self, x: f64, y: f64) {
        let imp = self.imp();

        let (inside, index, trailing) = self.index_at_xy(x, y);

        let mut cursor = "text";

        // If text selection initiated, redraw to show selection
        if imp.left_pressed.get() && imp.selection_start.get() != -1 {
            if trailing > 0 {
                imp.selection_end.set(index + 1);
            } else {
                imp.selection_end.set(index);
            }

            imp.draw_area.queue_draw();
        } else {
            // If no text selected, update cursor over links
            if self.is_link_at_index(inside, index).is_some() {
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

        motion_controller.connect_enter(clone!(@weak self as obj => move |_, x, y| {
            obj.set_cursor_motion(x, y);
        }));

        motion_controller.connect_motion(clone!(@weak self as obj => move |_, x, y| {
            obj.set_cursor_motion(x, y);
        }));

        imp.draw_area.add_controller(motion_controller);

        // Add click gesture controller
        let click_gesture = gtk::GestureClick::new();
        click_gesture.set_button(0);

        click_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

        click_gesture.connect_pressed(clone!(@weak self as obj, @weak imp => move |gesture, n, x, y| {
            let button = gesture.current_button();

            if button == gdk::BUTTON_PRIMARY {
                let link = obj.link_at_xy(x, y);

                if link.is_none() {
                    if n == 1 {
                        let (_, index, trailing) = obj.index_at_xy(x, y);

                        if trailing > 0 {
                            imp.selection_start.set(index + 1);
                        } else {
                            imp.selection_start.set(index);
                        }

                        imp.selection_end.set(-1);
                    } else if n == 2 {
                        let (_, index, _) = obj.index_at_xy(x, y);

                        let text = imp.pango_layout.get().unwrap().text();

                        let start = text[..index as usize]
                            .bytes()
                            .rposition(|ch: u8| ch.is_ascii_whitespace() || ch.is_ascii_punctuation())
                            .and_then(|start| Some(start + 1))
                            .unwrap_or(0);
                        let end = text[index as usize..]
                            .bytes()
                            .position(|ch: u8| ch.is_ascii_whitespace() || ch.is_ascii_punctuation())
                            .and_then(|end| Some(end + index as usize))
                            .unwrap_or(text.len());

                        imp.selection_start.set(start as i32);
                        imp.selection_end.set(end as i32);

                        imp.draw_area.queue_draw();
                    } else if n == 3 {
                        imp.selection_start.set(0);
                        imp.selection_end.set(imp.pango_layout.get().unwrap().text().len() as i32);

                        imp.draw_area.queue_draw();
                    }

                    imp.left_pressed.set(true);
                }

                imp.pressed_link.replace(link);
            } else if button == gdk::BUTTON_SECONDARY {
                // Enable/disable copy action
                let selection_start = imp.selection_start.get();
                let selection_end = imp.selection_end.get();

                let copy_action = imp.action_group.borrow().lookup_action("copy")
                    .and_downcast::<gio::SimpleAction>()
                    .expect("Must be a 'SimpleAction'");

                copy_action.set_enabled(selection_start != -1 && selection_end != -1 && selection_start != selection_end);

                // Show popover menu
                let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

                imp.popover_menu.set_pointing_to(Some(&rect));
                imp.popover_menu.popup();
            }
        }));

        click_gesture.connect_released(clone!(@weak self as obj, @weak imp => move |gesture, _, x, y| {
            let button = gesture.current_button();

            if button == gdk::BUTTON_PRIMARY {
                // Redraw if necessary to hide selection
                if imp.left_pressed.get() {
                    if imp.selection_end.get() == -1 || imp.selection_start.get() == imp.selection_end.get() {
                        imp.draw_area.queue_draw();
                    }
                }

                // Reset left button pressed
                imp.left_pressed.set(false);

                // Launch link if any
                if let Some(pressed_link) = imp.pressed_link.take() {
                    if let Some(link) = obj.link_at_xy(x, y).filter(|link| link == &pressed_link) {
                        if obj.emit_by_name::<bool>("link-activated", &[&link]) == false {
                            if let Ok(url) = Url::parse(&link) {
                                if let Some(handler) = gio::AppInfo::default_for_uri_scheme(url.scheme()) {
                                    let _res = handler.launch_uris(&[&link], None::<&gio::AppLaunchContext>);
                                }
                            }
                        }
                    }
                }
            }
        }));

        imp.draw_area.add_controller(click_gesture);
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Color scheme changed signal
        let style_manager = adw::StyleManager::default();

        style_manager.connect_dark_notify(clone!(@weak imp => move |style_manager| {
            // Update link/selected text colors when color scheme changes
            let link_btn = gtk::LinkButton::new("www.gtk.org");
            let label = gtk::Label::new(None);

            let css_provider = gtk::CssProvider::new();
            css_provider.load_from_string(&format!("label {{ color: alpha(@accent_color, 0.3); }}"));

            gtk::style_context_add_provider_for_display(&label.display(), &css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

            let btn_style = adw::StyleManager::for_display(&link_btn.display());
            let label_style = adw::StyleManager::for_display(&label.display());

            if style_manager.is_dark() {
                btn_style.set_color_scheme(adw::ColorScheme::ForceDark);
                label_style.set_color_scheme(adw::ColorScheme::ForceDark);
            } else {
                btn_style.set_color_scheme(adw::ColorScheme::ForceLight);
                label_style.set_color_scheme(adw::ColorScheme::ForceLight);
            }

            imp.link_rgba.replace(Some(link_btn.color()));

            imp.selection_bg_rgba.replace(Some(label.color()));

            gtk::style_context_remove_provider_for_display(&label.display(), &css_provider);
        }));
    }
}
