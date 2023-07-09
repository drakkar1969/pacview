use std::cell::{Cell, RefCell};

use gtk::{gio, glib, pango};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, once_cell::sync::OnceCell};
use pango::Underline;

use fancy_regex::Regex;
use lazy_static::lazy_static;
use url::Url;

use crate::info_pane::InfoPane;
use crate::prop_object::{PropObject, PropType};

//------------------------------------------------------------------------------
// MODULE: ValueRow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::ValueRow)]
    #[template(resource = "/com/github/PacView/ui/value_row.ui")]
    pub struct ValueRow {
        #[template_child]
        pub image: TemplateChild<gtk::Image>,
        #[template_child]
        pub view: TemplateChild<gtk::TextView>,
        #[template_child]
        pub buffer: TemplateChild<gtk::TextBuffer>,

        #[property(set = Self::set_text)]
        _text: RefCell<String>,

        pub infopane: OnceCell<InfoPane>,

        pub link_rgba: OnceCell<gtk::gdk::RGBA>,

        pub ptype: Cell<PropType>,

        pub bindings: RefCell<Vec<glib::Binding>>,

        pub hovering: Cell<bool>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for ValueRow {
        const NAME: &'static str = "ValueRow";
        type Type = super::ValueRow;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ValueRow {
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

            self.obj().setup_controllers();
        }
    }

    impl WidgetImpl for ValueRow {}
    impl BoxImpl for ValueRow {}

    impl ValueRow {
        //-----------------------------------
        // Text property custom setter
        //-----------------------------------
        fn set_text(&self, text: &str) {
            // Set TextView text
            match self.ptype.get() {
                PropType::Text => {
                    self.buffer.set_text(text);
                },
                PropType::Title => {
                    self.buffer.set_text(text);

                    self.add_bold_tag(0, -1);
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
                        static ref EXPR: Regex = Regex::new("^([^<]+)<([^>]+)>$").unwrap();
                    }

                    if let Ok(caps) = EXPR.captures(text) {
                        if let Some(caps) = caps.filter(|caps| caps.len() == 3) {
                            if let Some(m) = caps.get(2) {
                                let tag_name = format!("mailto:{}", &caps[2].to_string());

                                // Convert byte offsets to character offsets
                                self.add_link_tag(&tag_name, self.bytes_to_chars(text, m.start()), self.bytes_to_chars(text, m.end()));
                            }
                        }
                    }
                },
                PropType::LinkList => {
                    if text == "" {
                        self.buffer.set_text("None");
                    } else {
                        self.buffer.set_text(text);
        
                        lazy_static! {
                            static ref EXPR: Regex = Regex::new("(^|   )([a-zA-Z0-9@._+-]+)(?=<|>|=|:|   |$)").unwrap();
                        }

                        for caps in EXPR.captures_iter(text) {
                            if let Ok(caps) = caps {
                                if caps.len() >= 3 {
                                    if let Some(m) = caps.get(2) {
                                        let tag_name = format!("pkg://{}", &caps[2].to_string());

                                        self.add_link_tag(&tag_name, m.start() as i32, m.end() as i32);
                                    }
                                }
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
        // TextView tag helper functions
        //-----------------------------------
        fn add_tag(&self, tag: &gtk::TextTag, start: i32, end: i32) {
            let start_iter = self.buffer.iter_at_offset(start);

            let end_iter: gtk::TextIter;

            if end == -1 {
                end_iter = self.buffer.end_iter();
            } else {
                end_iter = self.buffer.iter_at_offset(end);
            }

            self.buffer.tag_table().add(tag);

            self.buffer.apply_tag(tag, &start_iter, &end_iter);
        }

        fn add_bold_tag(&self, start: i32, end: i32) {
            let tag = gtk::TextTag::builder()
                .weight(700)
                .build();

            self.add_tag(&tag, start, end);
        }

        fn add_link_tag(&self, text: &str, start: i32, end: i32) {
            let rgba = self.link_rgba.get().unwrap();

            if self.buffer.tag_table().lookup(text).is_none() {
                let tag = gtk::TextTag::builder()
                    .name(text)
                    .foreground_rgba(&rgba)
                    .underline(Underline::Single)
                    .build();

                self.add_tag(&tag, start, end);
            }
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: ValueRow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct ValueRow(ObjectSubclass<imp::ValueRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ValueRow {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(infopane: InfoPane, link_rgba: gtk::gdk::RGBA) -> Self {
        let row: Self = glib::Object::builder().build();

        let imp = row.imp();

        imp.infopane.set(infopane).unwrap();
        imp.link_rgba.set(link_rgba).unwrap();

        row
    }

    //-----------------------------------
    // Controller helper functions
    //-----------------------------------
    fn tag_at_xy(&self, x: i32, y: i32) -> Option<glib::GString> {
        let imp = self.imp();

        let (bx, by) = imp.view.window_to_buffer_coords(gtk::TextWindowType::Widget, x, y);

        if let Some(iter) = imp.view.iter_at_location(bx, by) {
            if iter.tags().len() > 0 {
                return iter.tags()[0].name()
            }
        }

        None
    }

    fn set_cursor_motion(&self, x: f64, y: f64) {
        let imp = self.imp();

        let hovering = self.tag_at_xy(x as i32, y as i32).is_some();

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

        let view = imp.view.get();

        // Change mouse pointer when hovering over links (add motion controller to view)
        let motion_controller = gtk::EventControllerMotion::new();

        motion_controller.connect_enter(clone!(@weak self as obj => move |_, x, y| {
            obj.set_cursor_motion(x, y);
        }));

        motion_controller.connect_motion(clone!(@weak self as obj => move |_, x, y| {
            obj.set_cursor_motion(x, y);
        }));

        view.add_controller(motion_controller.clone());

        // Activate links on click (add click gesture to view)
        let click_gesture = gtk::GestureClick::new();

        click_gesture.connect_released(clone!(@weak self as obj, @weak imp => move |_, _, x, y| {
            if let Some(link) = obj.tag_at_xy(x as i32, y as i32) {
                if let Ok(url) = Url::parse(&link) {
                    let url_scheme = url.scheme();

                    if url_scheme == "pkg" {
                        if let Some(pkg_name) = url.domain() {
                            let infopane = imp.infopane.get().unwrap();

                            infopane.link_handler(&pkg_name);
                        }
                    } else {
                        if let Some(handler) = gio::AppInfo::default_for_uri_scheme(url_scheme) {
                            let _res = handler.launch_uris(&[&link], None::<&gio::AppLaunchContext>);
                        }
                    }
                }
            }
        }));

        view.add_controller(click_gesture.clone());
    }

    //-----------------------------------
    // Public property binding functions
    //-----------------------------------
    pub fn bind_properties(&self, property: &PropObject) {
        let imp = self.imp();

        let image = imp.image.get();

        // Save property type (needed for text property setter)
        imp.ptype.replace(property.ptype());

        let mut bindings = imp.bindings.borrow_mut();

        // Bind PropObject properties to widget properties and save bindings
        let binding = property.bind_property("icon", &image, "visible")
            .transform_to(|_, icon: Option<&str>| Some(icon.is_some()))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        bindings.push(binding);

        let binding = property.bind_property("icon", &image, "icon-name")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        bindings.push(binding);

        let binding = property.bind_property("value", self, "text")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        bindings.push(binding);
    }

    pub fn unbind_properties(&self) {
        let imp = self.imp();

        // Unbind PropObject properties from widgets
        for binding in imp.bindings.borrow_mut().drain(..) {
            binding.unbind();
        }
    }
}
