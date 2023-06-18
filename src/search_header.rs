use std::cell::{Cell, RefCell};

use gtk::{glib, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;

use glib::subclass::Signal;
use glib::{clone, closure_local, once_cell::sync::Lazy};

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
// FLAGS: SearchFlags
//------------------------------------------------------------------------------
#[glib::flags(name = "SearchFlags")]
pub enum SearchFlags {
    NAME     = 0b00000001,
    DESC     = 0b00000010,
    GROUP    = 0b00000100,
    DEPS     = 0b00001000,
    OPTDEPS  = 0b00010000,
    PROVIDES = 0b00100000,
    FILES    = 0b01000000,
}

impl Default for SearchFlags {
    fn default() -> Self {
        SearchFlags::NAME
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
        pub tag_mode: TemplateChild<SearchTag>,

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
        pub clear_button: TemplateChild<gtk::Button>,

        #[property(get, set)]
        title: RefCell<Option<String>>,

        #[property(get, set)]
        capture_widget: RefCell<Option<gtk::Widget>>,
        #[property(get, set)]
        capture_controller: RefCell<gtk::EventControllerKey>,

        #[property(get, set)]
        active: Cell<bool>,

        #[property(get, set, builder(SearchMode::default()))]
        mode: Cell<SearchMode>,

        #[property(get, set)]
        flags: Cell<SearchFlags>,
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
                            SearchFlags::static_type(),
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

        // Set tag visibility
        imp.tag_name.set_visible(self.flags().contains(SearchFlags::NAME));
        imp.tag_desc.set_visible(self.flags().contains(SearchFlags::DESC));
        imp.tag_group.set_visible(self.flags().contains(SearchFlags::GROUP));
        imp.tag_deps.set_visible(self.flags().contains(SearchFlags::DEPS));
        imp.tag_optdeps.set_visible(self.flags().contains(SearchFlags::OPTDEPS));
        imp.tag_provides.set_visible(self.flags().contains(SearchFlags::PROVIDES));
        imp.tag_files.set_visible(self.flags().contains(SearchFlags::FILES));

        // Bind title property to title widget
        self.bind_property("title", &imp.title_widget.get(), "title")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

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
            if let Some(text) = glib::EnumValue::from_value(&header.mode().to_value())
                .and_then(|(_, enum_value)| Some(enum_value.nick().to_string()))
            {
                header.imp().tag_mode.set_text(text);

                header.emit_changed_signal();
            }
        });

        // Search flags property notify signal
        self.connect_flags_notify(|header| {
            let imp = header.imp();

            imp.tag_name.set_visible(header.flags().contains(SearchFlags::NAME));
            imp.tag_desc.set_visible(header.flags().contains(SearchFlags::DESC));
            imp.tag_group.set_visible(header.flags().contains(SearchFlags::GROUP));
            imp.tag_deps.set_visible(header.flags().contains(SearchFlags::DEPS));
            imp.tag_optdeps.set_visible(header.flags().contains(SearchFlags::OPTDEPS));
            imp.tag_provides.set_visible(header.flags().contains(SearchFlags::PROVIDES));
            imp.tag_files.set_visible(header.flags().contains(SearchFlags::FILES));

            header.emit_changed_signal();
        });

        // Search buffer text changed signal
        imp.search_buffer.connect_text_notify(clone!(@weak self as obj => move |_| {
            obj.emit_changed_signal();
        }));

        // Tags closed signals
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
            tag.connect_closure("closed", false, closure_local!(@watch self as obj => move |_: &SearchTag, text: &str| {
                let flags = obj.property("flags");

                let flags_class = glib::FlagsClass::new(SearchFlags::static_type()).unwrap();

                let flags = flags_class.builder_with_value(flags).unwrap()
                    .unset_by_nick(text)
                    .build()
                    .unwrap();

                obj.set_property("flags", flags);
            }));
        }

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
                &self.flags(),
                &self.mode()
            ]);
    }

    //-----------------------------------
    // Public set capture widget function
    //-----------------------------------
    pub fn set_key_capture_widget(&self, widget: &gtk::Widget) {
        if let Some(current_widget) = self.capture_widget() {
            current_widget.remove_controller(&self.capture_controller());
        }

        self.set_capture_widget(widget);

        let controller = gtk::EventControllerKey::new();

        self.set_capture_controller(&controller);

        controller.connect_key_pressed(clone!(@weak self as header => @default-return gtk::Inhibit(false), move |controller, _, _, state| {
            if !(state.contains(gdk::ModifierType::ALT_MASK) || state.contains(gdk::ModifierType::CONTROL_MASK))
            {
                if controller.forward(&header.imp().search_text.get()) {
                    header.set_active(true);
                }
            }

            gtk::Inhibit(false)
        }));

        widget.add_controller(controller);
    }
}
