use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gtk::{gio, glib, gdk, pango};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use glib::once_cell::sync::Lazy;
use glib::subclass::Signal;

use fancy_regex::Regex;
use lazy_static::lazy_static;
use url::Url;
use pangocairo;

use crate::prop_object::PropType;

//------------------------------------------------------------------------------
// MODULE: PropValueWidget
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PropValueWidget)]
    #[template(resource = "/com/github/PacView/ui/prop_value_widget.ui")]
    pub struct PropValueWidget {
        #[template_child]
        pub image: TemplateChild<gtk::Image>,
        #[template_child]
        pub draw_area: TemplateChild<gtk::DrawingArea>,

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(set = Self::set_icon, nullable)]
        _icon: RefCell<Option<String>>,
        #[property(set = Self::set_text)]
        _text: RefCell<String>,

        pub link_map: RefCell<HashMap<String, String>>,

        pub link_rgba: Cell<Option<gdk::RGBA>>,

        pub pango_layout: RefCell<Option<pango::Layout>>,

        pub hovering: Cell<bool>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PropValueWidget {
        const NAME: &'static str = "PropValueWidget";
        type Type = super::PropValueWidget;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PropValueWidget {
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

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_controllers();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for PropValueWidget {}
    impl BoxImpl for PropValueWidget {}

    impl PropValueWidget {
        //-----------------------------------
        // Icon property custom setter
        //-----------------------------------
        fn set_icon(&self, icon: Option<&str>) {
            if icon.is_some() {
                self.image.set_icon_name(icon);
            }

            self.image.set_visible(icon.is_some());
        }

        //-----------------------------------
        // Text property custom setter
        //-----------------------------------
        fn set_text(&self, text: &str) {
            let layout = self.pango_layout.borrow();
            let layout = layout.as_ref().unwrap();

            layout.set_text(text);
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PropValueWidget
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PropValueWidget(ObjectSubclass<imp::PropValueWidget>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PropValueWidget {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        let widget: Self = glib::Object::builder().build();

        let link_btn = gtk::LinkButton::new("www.gtk.org");

        widget.imp().link_rgba.replace(Some(link_btn.color()));

        widget
    }

    //-----------------------------------
    // Layout format helper functions
    //-----------------------------------
    fn format_text(&self, attrs: &pango::AttrList, start: usize, end: usize, weight: pango::Weight) {
        let color = self.color();

        let mut attr = pango::AttrColor::new_foreground((color.red() * 65535.0) as u16, (color.green() * 65535.0) as u16, (color.blue() * 65535.0) as u16);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attrs.insert(attr);

        let mut attr = pango::AttrInt::new_foreground_alpha((color.alpha() * 65535.0) as u16);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attrs.insert(attr);

        let mut attr = pango::AttrInt::new_weight(weight);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attrs.insert(attr);
    }

    fn format_link(&self, attrs: &pango::AttrList, start: usize, end: usize) {
        let color = self.imp().link_rgba.get().unwrap();

        let mut attr = pango::AttrColor::new_foreground((color.red() * 65535.0) as u16, (color.green() * 65535.0) as u16, (color.blue() * 65535.0) as u16);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attrs.insert(attr);

        let mut attr = pango::AttrInt::new_foreground_alpha((color.alpha() * 65535.0) as u16);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attrs.insert(attr);

        let mut attr = pango::AttrInt::new_underline(pango::Underline::Single);
        attr.set_start_index(start as u32);
        attr.set_end_index(end as u32);

        attrs.insert(attr);
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        let layout = imp.draw_area.create_pango_layout(None);
        layout.set_wrap(pango::WrapMode::Word);

        imp.pango_layout.replace(Some(layout));

        imp.draw_area.set_draw_func(clone!(@weak self as obj, @weak imp => move |_, context, width, _| {
            let layout = imp.pango_layout.borrow();
            let layout = layout.as_ref().unwrap();

            let mut link_map = imp.link_map.borrow_mut();

            link_map.clear();

            let attrs = pango::AttrList::new();

            // Set pango layout text text
            match obj.ptype() {
                PropType::Text => {
                    obj.format_text(&attrs, 0, layout.text().len(), pango::Weight::Normal);
                },
                PropType::Title => {
                    obj.format_text(&attrs, 0, layout.text().len(), pango::Weight::Bold);
                },
                PropType::Link => {
                    if layout.text().is_empty() {
                        layout.set_text("None");

                        obj.format_text(&attrs, 0, layout.text().len(), pango::Weight::Normal);
                    } else {
                        link_map.insert(layout.text().to_string(), layout.text().to_string());

                        obj.format_link(&attrs, 0, layout.text().len());
                    }
                },
                PropType::Packager => {
                    lazy_static! {
                        static ref EXPR: Regex = Regex::new("^(?:[^<]+?)<([^>]+?)>$").unwrap();
                    }

                    obj.format_text(&attrs, 0, layout.text().len(), pango::Weight::Normal);

                    if let Some(m) = EXPR.captures(&layout.text()).ok().flatten().and_then(|caps| caps.get(1)) {
                        link_map.insert(m.as_str().to_string(), format!("pkg://{}", m.as_str()));

                        obj.format_link(&attrs, m.start(), m.end());
                    }
                },
                PropType::LinkList => {
                    if layout.text().is_empty() {
                        layout.set_text("None");

                        obj.format_text(&attrs, 0, layout.text().len(), pango::Weight::Normal);
                    } else {
                        lazy_static! {
                            static ref EXPR: Regex = Regex::new("(?:^|     )([a-zA-Z0-9@._+-]+)(?=<|>|=|:|     |$)").unwrap();
                        }

                        obj.format_text(&attrs, 0, layout.text().len(), pango::Weight::Normal);

                        for caps in EXPR.captures_iter(&layout.text()).flatten() {
                            if let Some(m) = caps.get(1) {
                                link_map.insert(m.as_str().to_string(), format!("pkg://{}", m.as_str()));

                                obj.format_link(&attrs, m.start(), m.end());
                            }
                        }

                    }
                },
            }

            layout.set_attributes(Some(&attrs));

            layout.set_width(width * pango::SCALE);

            pangocairo::show_layout(&context, &layout);
        }));
    }

    //-----------------------------------
    // Controller helper functions
    //-----------------------------------
    fn is_link_at_xy(&self, x: i32, y: i32) -> bool {
        let imp = self.imp();

        let layout = imp.pango_layout.borrow();
        let layout = layout.as_ref().unwrap();

        let (inside, index, _) = layout.xy_to_index(x * pango::SCALE, y * pango::SCALE);

        if inside {
            if let Some(attrs) = layout.attributes() {
                if let Some(_) = attrs.attributes().into_iter()
                    .filter(|a| a.attr_class().type_() == pango::AttrType::Underline)
                    .find(|a| a.start_index() as i32 <= index && a.end_index() as i32 > index)
                {
                    return true
                }
            }
        }

        false
    }

    fn link_at_xy(&self, x: i32, y: i32) -> Option<String> {
        let imp = self.imp();

        let layout = imp.pango_layout.borrow();
        let layout = layout.as_ref().unwrap();

        let (inside, index, _) = layout.xy_to_index(x * pango::SCALE, y * pango::SCALE);

        if inside {
            if let Some(attrs) = layout.attributes() {
                if let Some(a) = attrs.attributes().into_iter()
                    .filter(|a| a.attr_class().type_() == pango::AttrType::Underline)
                    .find(|a| a.start_index() as i32 <= index && a.end_index() as i32 > index)
                {
                    let link_map = imp.link_map.borrow();

                    if let Some(link) = link_map.get(&layout.text()[a.start_index() as usize..a.end_index() as usize])
                    {
                        return Some(link.to_string())
                    }
                }
            }
        }

        None
    }

    fn set_cursor_motion(&self, x: f64, y: f64) {
        let imp = self.imp();

        let hovering = self.is_link_at_xy(x as i32, y as i32);

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
            if obj.is_link_at_xy(x as i32, y as i32) {
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
