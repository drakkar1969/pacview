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
        pub view: TemplateChild<gtk::TextView>,
        #[template_child]
        pub buffer: TemplateChild<gtk::TextBuffer>,

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(set = Self::set_icon, nullable)]
        _icon: RefCell<Option<String>>,
        #[property(set = Self::set_text)]
        _text: RefCell<String>,

        pub link_map: RefCell<HashMap<gtk::TextTag, String>>,

        pub link_rgba: OnceCell<gdk::RGBA>,

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

            obj.setup_controllers();
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
            // Set TextView text
            match self.obj().ptype() {
                PropType::Text => {
                    self.buffer.set_text(text);
                },
                PropType::Title => {
                    let title = format!("<b>{text}</b>");

                    let mut iter = self.buffer.start_iter();

                    self.buffer.insert_markup(&mut iter, &title);
                },
                PropType::Link => {
                    if text == "" {
                        self.buffer.set_text("None");
                    } else {
                        self.buffer.set_text(text);

                        self.add_link_tag(text, 0, -1);
                    }
                },
                PropType::Packager => {
                    self.buffer.set_text(text);

                    lazy_static! {
                        static ref EXPR: Regex = Regex::new("^(?:[^<]+?)<([^>]+?)>$").unwrap();
                    }

                    if let Ok(caps) = EXPR.captures(text) {
                        if let Some(m) = caps.and_then(|caps| caps.get(1)) {
                            // Convert byte offsets to character offsets
                            self.add_link_tag(
                                &format!("mailto:{}", m.as_str()),
                                self.bytes_to_chars(text, m.start()),
                                self.bytes_to_chars(text, m.end())
                            );
                        }
                    }
                },
                PropType::LinkList => {
                    if text == "" {
                        self.buffer.set_text("None");
                    } else {
                        self.buffer.set_text(text);
        
                        lazy_static! {
                            static ref EXPR: Regex = Regex::new("(?:^|     )([a-zA-Z0-9@._+-]+)(?=<|>|=|:|     |$)").unwrap();
                        }

                        for caps in EXPR.captures_iter(text) {
                            if let Some(m) = caps.ok().and_then(|caps| caps.get(1)) {
                                self.add_link_tag(
                                    &format!("pkg://{}", m.as_str()),
                                    m.start() as i32,
                                    m.end() as i32
                                );
                            }
                        }
                    }
                },
            }
        }

        //-----------------------------------
        // Bytes to chars helper function
        //-----------------------------------
        fn bytes_to_chars(&self, text: &str, bytes: usize) -> i32 {
            text[0..bytes].chars().count() as i32
        }

        //-----------------------------------
        // TextView tag helper function
        //-----------------------------------
        fn add_link_tag(&self, link: &str, start: i32, end: i32) {
            // Create tag
            let tag = gtk::TextTag::builder()
                .foreground_rgba(self.link_rgba.get().unwrap())
                .underline(pango::Underline::Single)
                .build();

            self.buffer.tag_table().add(&tag);

            // Apply tag
            let start_iter = self.buffer.iter_at_offset(start);

            let end_iter = if end == -1 {
                self.buffer.end_iter()
            } else {
                self.buffer.iter_at_offset(end)
            };

            self.buffer.apply_tag(&tag, &start_iter, &end_iter);

            // Save tag in link map
            let mut link_map = self.link_map.borrow_mut();

            link_map.insert(tag, link.to_string());
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
    pub fn new(link_rgba: gdk::RGBA) -> Self {
        let widget: Self = glib::Object::builder().build();

        widget.imp().link_rgba.set(link_rgba).unwrap();

        widget
    }

    //-----------------------------------
    // Controller helper functions
    //-----------------------------------
    fn is_tag_at_xy(&self, x: i32, y: i32) -> bool {
        let imp = self.imp();

        let (bx, by) = imp.view.window_to_buffer_coords(gtk::TextWindowType::Widget, x, y);

        imp.view.iter_at_location(bx, by).filter(|iter| !iter.tags().is_empty()).is_some()
    }

    fn tag_at_xy(&self, x: i32, y: i32) -> Option<String> {
        let imp = self.imp();

        let (bx, by) = imp.view.window_to_buffer_coords(gtk::TextWindowType::Widget, x, y);

        if let Some(iter) = imp.view.iter_at_location(bx, by).filter(|iter| !iter.tags().is_empty())
        {
            if let Some(link) = imp.link_map.borrow().get(&iter.tags()[0]) {
                return Some(link.to_string())
            }
        }

        None
    }

    fn set_cursor_motion(&self, x: f64, y: f64) {
        let imp = self.imp();

        let hovering = self.is_tag_at_xy(x as i32, y as i32);

        if hovering != imp.hovering.get() {
            imp.hovering.replace(hovering);

            if hovering {
                imp.view.set_cursor_from_name(Some("pointer"));
            } else {
                imp.view.set_cursor_from_name(Some("text"));
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

        imp.view.add_controller(motion_controller);

        // Activate links on click (add click gesture to view)
        let click_gesture = gtk::GestureClick::new();

        click_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

        click_gesture.connect_pressed(clone!(@weak self as obj => move |gesture, _, x, y| {
            if obj.is_tag_at_xy(x as i32, y as i32) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        }));

        click_gesture.connect_released(clone!(@weak self as obj => move |_, _, x, y| {
            if let Some(link) = obj.tag_at_xy(x as i32, y as i32) {
                if obj.emit_by_name::<bool>("link-activated", &[&link]) == false {
                    if let Ok(url) = Url::parse(&link) {
                        if let Some(handler) = gio::AppInfo::default_for_uri_scheme(url.scheme()) {
                            let _res = handler.launch_uris(&[&link], None::<&gio::AppLaunchContext>);
                        }
                    }
                }
            }
        }));

        imp.view.add_controller(click_gesture);
    }
}
