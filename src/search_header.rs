use std::cell::{Cell, RefCell};

use gtk::{glib, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;

use glib::subclass::Signal;
use glib::{clone, once_cell::sync::Lazy};

use crate::search_tag::SearchTag;

//------------------------------------------------------------------------------
// ENUM: SearchMode
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchMode")]
pub enum SearchMode {
    #[enum_value(name = "All")]
    All = 0,
    #[enum_value(name = "Any")]
    Any = 1,
    #[enum_value(name = "Exact")]
    Exact = 2,
}

impl Default for SearchMode {
    fn default() -> Self {
        SearchMode::All
    }
}

//------------------------------------------------------------------------------
// MODULE: SearchHeader
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::SearchHeader)]
    #[template(resource = "/com/github/PacView/ui/search_header.ui")]
    pub struct SearchHeader {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub title_widget: TemplateChild<adw::WindowTitle>,

        #[template_child]
        pub search_text: TemplateChild<gtk::Text>,
        #[template_child]
        pub search_buffer: TemplateChild<gtk::EntryBuffer>,

        #[template_child]
        pub tag_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub tag_name: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_desc: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_group: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_deps: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_optdeps: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_provides: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_files: TemplateChild<SearchTag>,
        #[template_child]

        pub tag_all: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_any: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_exact: TemplateChild<SearchTag>,

        #[template_child]
        pub clear_button: TemplateChild<gtk::Button>,

        #[property(get, set)]
        title: RefCell<Option<String>>,

        #[property(get, set)]
        capture_widget: RefCell<Option<gtk::Widget>>,
        #[property(get, set)]
        capture_controller: RefCell<Option<gtk::EventControllerKey>>,

        #[property(get, set)]
        active: Cell<bool>,

        #[property(get, set, builder(SearchMode::default()))]
        mode: Cell<SearchMode>,

        #[property(get, set)]
        by_name: Cell<bool>,
        #[property(get, set)]
        by_desc: Cell<bool>,
        #[property(get, set)]
        by_group: Cell<bool>,
        #[property(get, set)]
        by_deps: Cell<bool>,
        #[property(get, set)]
        by_optdeps: Cell<bool>,
        #[property(get, set)]
        by_provides: Cell<bool>,
        #[property(get, set)]
        by_files: Cell<bool>,

        #[property(get, set)]
        block_notify: Cell<bool>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SearchHeader {
        const NAME: &'static str = "SearchHeader";
        type Type = super::SearchHeader;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            SearchTag::ensure_type();
            SearchMode::ensure_type();

            klass.bind_template();
            klass.set_layout_manager_type::<gtk::BoxLayout>();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SearchHeader {
        //-----------------------------------
        // Custom signals
        //-----------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("changed")
                        .param_types([
                            String::static_type(),
                            bool::static_type(),
                            bool::static_type(),
                            bool::static_type(),
                            bool::static_type(),
                            bool::static_type(),
                            bool::static_type(),
                            bool::static_type(),
                            SearchMode::static_type()])
                        .build(),
                    Signal::builder("activated")
                        .param_types([bool::static_type()])
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
            obj.setup_signals();
        }
    }

    impl WidgetImpl for SearchHeader {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SearchHeader
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchHeader(ObjectSubclass<imp::SearchHeader>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl SearchHeader {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind title property to title widget
        self.bind_property("title", &imp.title_widget.get(), "title")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind search mode property to search mode tag visibility
        self.bind_property("mode", &imp.tag_all.get(), "visible")
            .transform_to(|_, mode: SearchMode| Some(mode == SearchMode::All))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        self.bind_property("mode", &imp.tag_any.get(), "visible")
            .transform_to(|_, mode: SearchMode| Some(mode == SearchMode::Any))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        self.bind_property("mode", &imp.tag_exact.get(), "visible")
            .transform_to(|_, mode: SearchMode| Some(mode == SearchMode::Exact))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind search by-* properties to search tag visibility
        let tag_array = [
            imp.tag_name.get(),
            imp.tag_desc.get(),
            imp.tag_group.get(),
            imp.tag_deps.get(),
            imp.tag_optdeps.get(),
            imp.tag_provides.get(),
            imp.tag_files.get(),
        ];

        for tag in tag_array {
            if let Some(text) = tag.text() {
                let prop_name = format!("by-{}", text);

                self.bind_property(&prop_name, &tag, "visible")
                    .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                    .build();
            }
        }

        // Bind search text to clear button visibility
        imp.search_buffer.bind_property("text", &imp.clear_button.get(), "visible")
            .transform_to(|_, text: &str| Some(text != ""))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();
        
        // Search active property notify signal
        self.connect_active_notify(|header| {
            let imp = header.imp();

            if header.active() {
                imp.stack.set_visible_child_name("search");

                imp.search_text.grab_focus_without_selecting();
            } else {
                imp.search_text.set_text("");

                imp.stack.set_visible_child_name("title");
            }

            header.emit_by_name::<()>("activated", &[&header.active()]);
        });

        // Search mode property notify signal
        self.connect_mode_notify(|header| {
            header.emit_changed_signal();
        });

        // Search by-* properties notify signals
        let tag_array = [
            imp.tag_name.get(),
            imp.tag_desc.get(),
            imp.tag_group.get(),
            imp.tag_deps.get(),
            imp.tag_optdeps.get(),
            imp.tag_provides.get(),
            imp.tag_files.get(),
        ];

        for tag in tag_array {
            if let Some(text) = tag.text() {
                let prop_name = format!("by-{}", text);

                self.connect_notify(Some(&prop_name), |header, _| {
                    if !header.block_notify() {
                        header.emit_changed_signal();
                    }
                });
            }
        }

        // Block notify property notify signal
        self.connect_block_notify_notify(|header| {
            if header.block_notify() == false {
                header.emit_changed_signal();
            }
        });

        // Search buffer text changed signal
        imp.search_buffer.connect_text_notify(clone!(@weak self as obj => move |_| {
            obj.emit_changed_signal();
        }));

        // Clear button clicked signal
        imp.clear_button.connect_clicked(clone!(@weak imp => move |_| {
            imp.search_buffer.set_text("");
        }));
    }

    //-----------------------------------
    // Emit changed signal helper function
    //-----------------------------------
    fn emit_changed_signal(&self) {
        let imp = self.imp();

        self.emit_by_name::<()>("changed",
            &[
                &imp.search_buffer.text(),
                &self.by_name(),
                &self.by_desc(),
                &self.by_group(),
                &self.by_deps(),
                &self.by_optdeps(),
                &self.by_provides(),
                &self.by_files(),
                &self.mode()
            ]);
    }

    //-----------------------------------
    // Public set capture widget function
    //-----------------------------------
    pub fn set_key_capture_widget(&self, widget: &gtk::Widget) {
        if let Some(current_widget) = self.capture_widget() {
            current_widget.remove_controller(&self.capture_controller().unwrap());
        }

        self.set_capture_widget(widget);

        let controller = gtk::EventControllerKey::new();

        self.set_capture_controller(&controller);

        let exclude_keys = [
            gdk::Key::Tab, gdk::Key::Caps_Lock, gdk::Key::Num_Lock, gdk::Key::F1, gdk::Key::F2,
            gdk::Key::F3, gdk::Key::F4, gdk::Key::F5, gdk::Key::F6, gdk::Key::F7, gdk::Key::F8,
            gdk::Key::F9, gdk::Key::F10, gdk::Key::F11, gdk::Key::F12, gdk::Key::BackSpace,
            gdk::Key::Delete, gdk::Key::KP_Delete, gdk::Key::Insert, gdk::Key::KP_Insert,
            gdk::Key::Shift_L, gdk::Key::Shift_R, gdk::Key::Control_L, gdk::Key::Control_R,
            gdk::Key::Alt_L, gdk::Key::Alt_R, gdk::Key::KP_Begin, gdk::Key::ISO_Level3_Shift
        ];

        controller.connect_key_pressed(clone!(@weak self as header => @default-return gtk::Inhibit(false), move |controller, key, _, state| {
            if !(state.contains(gdk::ModifierType::ALT_MASK) ||
                 state.contains(gdk::ModifierType::CONTROL_MASK) ||
                 exclude_keys.contains(&key)) {
                if controller.forward(&header.imp().search_text.get()) {
                    header.set_active(true);
                }
            }

            gtk::Inhibit(false)
        }));

        widget.add_controller(controller);
    }
}
