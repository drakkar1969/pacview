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

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(set = Self::set_text)]
        _text: RefCell<String>,

        pub link_rgba: Cell<Option<gdk::RGBA>>,

        pub pango_layout: OnceCell<pango::Layout>,

        pub link_map: RefCell<HashMap<String, String>>,

        pub hovering: Cell<bool>,
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
        // Default property functions
        //-----------------------------------
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let link_btn = gtk::LinkButton::new("www.gtk.org");

            self.link_rgba.replace(Some(link_btn.color()));

            let obj = self.obj();

            obj.setup_widgets();
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
    fn format_text(&self, attr_list: &pango::AttrList, start: usize, end: usize, weight: pango::Weight) {
        let color = self.color();

        let bg_color = self.parent().unwrap().color();

        let red = ((1.0 - color.alpha()) * bg_color.red()) + (color.red() * color.alpha());
        let green = ((1.0 - color.alpha()) * bg_color.green()) + (color.green() * color.alpha());
        let blue = ((1.0 - color.alpha()) * bg_color.blue()) + (color.blue() * color.alpha());

        let mut attr = pango::AttrColor::new_foreground((red * 65535.0) as u16, (green * 65535.0) as u16, (blue * 65535.0) as u16);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);

        let mut attr = pango::AttrInt::new_weight(weight);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);
    }

    fn format_link(&self, attr_list: &pango::AttrList, start: usize, end: usize) {
        let color = self.imp().link_rgba.get().unwrap();

        let bg_color = self.parent().unwrap().color();

        let red = ((1.0 - color.alpha()) * bg_color.red()) + (color.red() * color.alpha());
        let green = ((1.0 - color.alpha()) * bg_color.green()) + (color.green() * color.alpha());
        let blue = ((1.0 - color.alpha()) * bg_color.blue()) + (color.blue() * color.alpha());

        let mut attr = pango::AttrColor::new_foreground((red * 65535.0) as u16, (green * 65535.0) as u16, (blue * 65535.0) as u16);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);

        let mut attr = pango::AttrInt::new_foreground_alpha((color.alpha() * 65535.0) as u16);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);

        let mut attr = pango::AttrInt::new_underline(pango::Underline::Single);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attr_list.insert(attr);
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        let layout = imp.draw_area.create_pango_layout(None);
        layout.set_wrap(pango::WrapMode::Word);
        layout.set_line_spacing(1.15);

        imp.pango_layout.set(layout).unwrap();

        imp.draw_area.set_draw_func(clone!(@weak self as obj, @weak imp => move |_, context, _, _| {
            let layout = imp.pango_layout.get().unwrap();

            let mut link_map = imp.link_map.borrow_mut();

            link_map.clear();

            let attr_list = pango::AttrList::new();

            // Set pango layout text text
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

            layout.set_attributes(Some(&attr_list));

            pangocairo::show_layout(&context, &layout);
        }));
    }

    //-----------------------------------
    // Controller helper functions
    //-----------------------------------
    fn is_link_at_xy(&self, x: i32, y: i32) -> Option<pango::Attribute> {
        let imp = self.imp();

        let layout = imp.pango_layout.get().unwrap();

        let (inside, index, _) = layout.xy_to_index(x * pango::SCALE, y * pango::SCALE);

        if inside {
            return layout.attributes().and_then(|attr_list| {
                attr_list.attributes().into_iter()
                    .find(|attr| attr.attr_class().type_() == pango::AttrType::Underline && attr.start_index() as i32 <= index && attr.end_index() as i32 > index)
            })
        }

        None
    }

    fn link_at_xy(&self, x: i32, y: i32) -> Option<String> {
        let imp = self.imp();

        if let Some(attr) = self.is_link_at_xy(x, y) {
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

        let hovering = self.is_link_at_xy(x as i32, y as i32).is_some();

        if hovering != imp.hovering.get() {
            imp.hovering.replace(hovering);

            if hovering {
                imp.draw_area.set_cursor_from_name(Some("pointer"));
            } else {
                imp.draw_area.set_cursor_from_name(Some("default"));
            }
        }
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Change mouse pointer when hovering over links (add motion controller to view)
        let motion_controller = gtk::EventControllerMotion::new();

        motion_controller.connect_enter(clone!(@weak self as obj => move |_, x, y| {
            obj.set_cursor_motion(x, y);
        }));

        motion_controller.connect_motion(clone!(@weak self as obj => move |_, x, y| {
            obj.set_cursor_motion(x, y);
        }));

        imp.draw_area.add_controller(motion_controller);

        // Activate links on click (add click gesture to view)
        let click_gesture = gtk::GestureClick::new();

        click_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

        click_gesture.connect_pressed(clone!(@weak self as obj => move |gesture, _, x, y| {
            if obj.is_link_at_xy(x as i32, y as i32).is_some() {
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        }));

        click_gesture.connect_released(clone!(@weak self as obj => move |_, _, x, y| {
            if let Some(link) = obj.link_at_xy(x as i32, y as i32) {
                if obj.emit_by_name::<bool>("link-activated", &[&link]) == false {
                    if let Ok(url) = Url::parse(&link) {
                        if let Some(handler) = gio::AppInfo::default_for_uri_scheme(url.scheme()) {
                            let _res = handler.launch_uris(&[&link], None::<&gio::AppLaunchContext>);
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
            // Update link color when color scheme changes
            let link_btn = gtk::LinkButton::new("www.gtk.org");

            let btn_style = adw::StyleManager::for_display(&link_btn.display());

            if style_manager.is_dark() {
                btn_style.set_color_scheme(adw::ColorScheme::ForceDark);
            } else {
                btn_style.set_color_scheme(adw::ColorScheme::ForceLight);
            }

            imp.link_rgba.replace(Some(link_btn.color()));
        }));
    }
}
