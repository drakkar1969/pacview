use std::cell::{Cell, RefCell};
use core::time::Duration;

use gtk::{glib, gio, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::{clone, closure_local};
use glib::once_cell::sync::Lazy;

use crate::search_tag::SearchTag;

//------------------------------------------------------------------------------
// ENUM: SearchMode
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchMode")]
pub enum SearchMode {
    All = 0,
    Any = 1,
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

        pub capture_widget: RefCell<Option<gtk::Widget>>,
        pub capture_controller: RefCell<gtk::EventControllerKey>,

        #[property(get, set, nullable)]
        title: RefCell<Option<String>>,

        #[property(get, set)]
        active: Cell<bool>,

        #[property(get, set, builder(SearchMode::default()))]
        mode: Cell<SearchMode>,

        #[property(get, set, default = SearchFlags::default(), construct)]
        flags: Cell<SearchFlags>,

        #[property(get, set, default = 150, construct)]
        delay: Cell<u64>,

        pub delay_source_id: RefCell<Option<glib::SourceId>>,
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
            obj.setup_actions();
        }

        //-----------------------------------
        // Dispose function
        //-----------------------------------
        fn dispose(&self) {
            self.dispose_template();
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

        // Bind search text to clear button visibility
        imp.search_text.bind_property("text", &imp.clear_button.get(), "visible")
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
            if let Some((_, value)) = glib::EnumValue::from_value(&header.mode().to_value()) {
                header.imp().tag_mode.set_text(Some(value.nick()));

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

        // Search text changed signal
        imp.search_text.connect_changed(clone!(@weak self as obj, @weak imp => move |search_text| {
            // Remove delay timer if present
            if let Some(delay_id) = imp.delay_source_id.take() {
                delay_id.remove();
            }

            if search_text.text() == "" {
                obj.emit_changed_signal();
            } else {
                // Start delay timer
                let delay_id = glib::timeout_add_local_once(
                    Duration::from_millis(obj.delay()),
                    clone!(@weak imp => move || {
                        obj.emit_changed_signal();

                        imp.delay_source_id.take();
                    })
                );

                imp.delay_source_id.replace(Some(delay_id));
            }
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

                if let Some(flags) = flags_class.builder_with_value(flags).unwrap()
                    .unset_by_nick(text)
                    .build()
                {
                    obj.set_property("flags", flags);
                }
            }));
        }

        // Clear button clicked signal
        imp.clear_button.connect_clicked(clone!(@weak imp => move |_| {
            imp.search_text.set_text("");
        }));
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        // Create search action group
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        // Create shortcut controller
        let controller = gtk::ShortcutController::new();

        // Add search mode stateful action
        let mode_action = gio::SimpleAction::new_stateful("set-mode", Some(&String::static_variant_type()), "all".to_variant());

        mode_action.connect_change_state(clone!(@weak self as obj => move |action, param| {
            let param = param
                .expect("Must be a 'Variant'")
                .get::<String>()
                .expect("Must be a 'String'");

            match param.as_str() {
                "all" => {
                    obj.set_mode(SearchMode::All);
                    action.set_state(param.to_variant());
                },
                "any" => {
                    obj.set_mode(SearchMode::Any);
                    action.set_state(param.to_variant());
                },
                "exact" => {
                    obj.set_mode(SearchMode::Exact);
                    action.set_state(param.to_variant());
                },
                _ => unreachable!()
            }
        }));

        search_group.add_action(&mode_action);

        // Add cycle search mode shortcut
        let cycle_action = gtk::CallbackAction::new(
            clone!(@weak search_group => @default-return true, move |_, _| {
                if let Some(mode_action) = search_group.lookup_action("set-mode") {
                    let state = mode_action.state()
                        .expect("Must be a 'Variant'")
                        .get::<String>()
                        .expect("Must be a 'String'");

                    match state.as_str() {
                        "all" => mode_action.change_state(&"any".to_variant()),
                        "any" => mode_action.change_state(&"exact".to_variant()),
                        "exact" => mode_action.change_state(&"all".to_variant()),
                        _ => unreachable!()
                    };
                }

                true
            })
        );

        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>M"),
            Some(cycle_action))
        );

        // Add select all search flags action/shortcut
        let all_action = gio::SimpleAction::new("all-flags", None);

        all_action.connect_activate(clone!(@weak self as obj => move |_, _| {
            obj.set_flags(SearchFlags::all());
        }));

        search_group.add_action(&all_action);

        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>L"),
            Some(gtk::NamedAction::new("search.all-flags"))
        ));

        // Add reset search flags action/shortcut
        let reset_action = gio::SimpleAction::new("reset-flags", None);

        reset_action.connect_activate(clone!(@weak self as obj => move |_, _| {
            obj.set_flags(SearchFlags::NAME);
        }));

        search_group.add_action(&reset_action);

        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>R"),
            Some(gtk::NamedAction::new("search.reset-flags"))
        ));

        // Add search flags stateful actions/shortcuts
        let flags_class = glib::FlagsClass::new(SearchFlags::static_type()).unwrap();

        for (i, f) in flags_class.values().iter().enumerate() {
            let flag = SearchFlags::from_bits_truncate(f.value());

            // Create stateful action
            let flag_action = gio::SimpleAction::new_stateful(&format!("flag-{}", f.nick()), None, (flag == SearchFlags::NAME).to_variant());

            flag_action.connect_activate(clone!(@weak self as obj, @strong flag => move |_, _| {
                obj.set_flags(obj.flags() ^ flag);
            }));

            search_group.add_action(&flag_action);

            let named_action = gtk::NamedAction::new(&format!("search.flag-{}", f.nick()));

            controller.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(&format!("<ctrl>{}", i+1)),
                Some(named_action))
            );
    
            // Bind search header flags property to action state
            self.bind_property("flags", &flag_action, "state")
                .transform_to(move |_, flags: SearchFlags| Some(flags.contains(flag).to_variant()))
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        }

        // Add shortcut controller to search header
        self.add_controller(controller);
    }

    //-----------------------------------
    // Emit changed signal helper function
    //-----------------------------------
    fn emit_changed_signal(&self) {
        let imp = self.imp();

        self.emit_by_name::<()>("changed",
            &[
                &imp.search_text.text(),
                &self.flags(),
                &self.mode()
            ]);
    }

    //-----------------------------------
    // Public set capture widget function
    //-----------------------------------
    pub fn set_key_capture_widget(&self, widget: gtk::Widget) {
        let imp = self.imp();

        if let Some(current_widget) = &*imp.capture_widget.borrow() {
            current_widget.remove_controller(&*imp.capture_controller.borrow());
        }

        let controller = gtk::EventControllerKey::new();

        controller.connect_key_pressed(clone!(@weak self as header => @default-return gtk::Inhibit(false), move |controller, _, _, state| {
            if !(state.contains(gdk::ModifierType::ALT_MASK) || state.contains(gdk::ModifierType::CONTROL_MASK))
            {
                if controller.forward(&header.imp().search_text.get()) {
                    header.set_active(true);
                }
            }

            gtk::Inhibit(false)
        }));

        widget.add_controller(controller.clone());

        imp.capture_widget.replace(Some(widget));

        imp.capture_controller.replace(controller);
    }
}
