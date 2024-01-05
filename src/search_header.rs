use std::cell::{Cell, RefCell};
use core::time::Duration;

use gtk::{glib, gio, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::clone;
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
// ENUM: SearchType
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchType")]
pub enum SearchType {
    Name = 0,
    Desc = 1,
    Group = 2,
    Deps = 3,
    Optdeps = 4,
    Provides = 5,
    Files = 6,
}

impl Default for SearchType {
    fn default() -> Self {
        SearchType::Name
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
        pub icon_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,

        #[template_child]
        pub tag_aur: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_mode: TemplateChild<SearchTag>,
        #[template_child]
        pub tag_type: TemplateChild<SearchTag>,

        #[template_child]
        pub search_text: TemplateChild<gtk::Text>,

        #[template_child]
        pub clear_button: TemplateChild<gtk::Button>,

        pub capture_widget: RefCell<Option<gtk::Widget>>,
        pub capture_controller: RefCell<gtk::EventControllerKey>,

        #[property(get, set, nullable)]
        title: RefCell<Option<String>>,

        #[property(get, set)]
        enabled: Cell<bool>,

        #[property(get, set, builder(SearchMode::default()))]
        mode: Cell<SearchMode>,
        #[property(get, set, builder(SearchType::default()))]
        stype: Cell<SearchType>,

        #[property(get, set)]
        include_aur: Cell<bool>,
        #[property(get, set, default = 150, construct)]
        delay: Cell<u64>,

        pub delay_source_id: RefCell<Option<glib::SourceId>>,

        pub search_action_group: RefCell<Option<gio::SimpleActionGroup>>
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

    #[glib::derived_properties]
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
                            SearchMode::static_type(),
                            SearchType::static_type(),
                            bool::static_type(),
                        ])
                        .build(),
                    Signal::builder("enabled")
                        .param_types([bool::static_type()])
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

            obj.setup_widgets();
            obj.setup_signals();
            obj.setup_actions();
            obj.setup_shortcuts();
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
        
        // Search enabled property notify signal
        self.connect_enabled_notify(|header| {
            let imp = header.imp();

            if header.enabled() {
                imp.stack.set_visible_child_name("search");

                imp.search_text.grab_focus_without_selecting();
            } else {
                imp.search_text.set_text("");

                imp.stack.set_visible_child_name("title");
            }

            header.emit_by_name::<()>("enabled", &[&header.enabled()]);
        });

        // Include AUR property notify signal
        self.connect_include_aur_notify(|header| {
            let imp = header.imp();

            imp.tag_aur.set_visible(header.include_aur());

            imp.search_text.set_text("");

            if let Some(group) = &*imp.search_action_group.borrow() {
                if let Some(mode_action) = group.lookup_action("set-mode") {
                    mode_action.change_state(&"all".to_variant());
                }

                if let Some(type_action) = group.lookup_action("set-type") {
                    type_action.change_state(&"name".to_variant());
                }
            }
        });

        // Search mode property notify signal
        self.connect_mode_notify(|header| {
            if let Some((_, value)) = glib::EnumValue::from_value(&header.mode().to_value()) {
                header.imp().tag_mode.set_text(Some(value.nick()));

                header.emit_changed_signal();
            }
        });

        // Search type property notify signal
        self.connect_stype_notify(|header| {
            if let Some((_, value)) = glib::EnumValue::from_value(&header.stype().to_value()) {
                header.imp().tag_type.set_text(Some(value.nick()));

                header.emit_changed_signal();
            }
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
                if obj.include_aur() == false {
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
            }
        }));

        // Search text activate signal
        imp.search_text.connect_activate(clone!(@weak self as obj => move |search_text| {
            if obj.include_aur() == true && search_text.text() != "" {
                obj.emit_changed_signal();
            }
        }));

        // Clear button clicked signal
        imp.clear_button.connect_clicked(clone!(@weak imp => move |_| {
            imp.search_text.set_text("");
        }));
    }

    //-----------------------------------
    // Emit changed signal helper function
    //-----------------------------------
    fn emit_changed_signal(&self) {
        let imp = self.imp();

        self.emit_by_name::<()>("changed",
            &[
                &imp.search_text.text(),
                &self.mode(),
                &self.stype(),
                &self.include_aur()
            ]);
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        // Add include AUR property action
        let aur_action = gio::PropertyAction::new("include-aur", self, "include-aur");

        // Add search mode stateful action
        let mode_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("set-mode")
            .parameter_type(Some(&String::static_variant_type()))
            .state("all".to_variant())
            .change_state(clone!(@weak self as obj => move |_, action, param| {
                let param = param
                    .expect("Must be a 'Variant'")
                    .get::<String>()
                    .expect("Must be a 'String'");
    
                match param.as_str() {
                    "all" => {
                        obj.set_mode(SearchMode::All);
                        action.set_state(&param.to_variant());
                    },
                    "any" => {
                        obj.set_mode(SearchMode::Any);
                        action.set_state(&param.to_variant());
                    },
                    "exact" => {
                        obj.set_mode(SearchMode::Exact);
                        action.set_state(&param.to_variant());
                    },
                    _ => unreachable!()
                }
            }))
            .build();

        // Add cycle search mode action
        let cycle_mode_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("cycle-mode")
            .activate(|group, _, _| {
                if let Some(mode_action) = group.lookup_action("set-mode") {
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
            })
            .build();

        // Add reverse cycle search mode action
        let reverse_mode_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("rev-cycle-mode")
            .activate(|group, _, _| {
                if let Some(mode_action) = group.lookup_action("set-mode") {
                    let state = mode_action.state()
                        .expect("Must be a 'Variant'")
                        .get::<String>()
                        .expect("Must be a 'String'");

                    match state.as_str() {
                        "all" => mode_action.change_state(&"exact".to_variant()),
                        "any" => mode_action.change_state(&"all".to_variant()),
                        "exact" => mode_action.change_state(&"any".to_variant()),
                        _ => unreachable!()
                    };
                }
            })
            .build();

        // Add search mode letter actions
        let mode_all_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("set-mode-all")
            .activate(move |group, _, _| {
                if let Some(mode_action) = group.lookup_action("set-mode") {
                    mode_action.change_state(&"all".to_variant());
                }
            })
            .build();

        let mode_any_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("set-mode-any")
            .activate(move |group, _, _| {
                if let Some(mode_action) = group.lookup_action("set-mode") {
                    mode_action.change_state(&"any".to_variant());
                }
            })
            .build();

        let mode_exact_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("set-mode-exact")
            .activate(move |group, _, _| {
                if let Some(mode_action) = group.lookup_action("set-mode") {
                    mode_action.change_state(&"exact".to_variant());
                }
            })
            .build();

        // Add search type stateful action
        let type_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("set-type")
            .parameter_type(Some(&String::static_variant_type()))
            .state("name".to_variant())
            .change_state(clone!(@weak self as obj => move |_, action, param| {
                let param = param
                    .expect("Must be a 'Variant'")
                    .get::<String>()
                    .expect("Must be a 'String'");
    
                match param.as_str() {
                    "name" => {
                        obj.set_stype(SearchType::Name);
                        action.set_state(&param.to_variant());
                    },
                    "desc" => {
                        obj.set_stype(SearchType::Desc);
                        action.set_state(&param.to_variant());
                    },
                    "group" => {
                        obj.set_stype(SearchType::Group);
                        action.set_state(&param.to_variant());
                    },
                    "deps" => {
                        obj.set_stype(SearchType::Deps);
                        action.set_state(&param.to_variant());
                    },
                    "optdeps" => {
                        obj.set_stype(SearchType::Optdeps);
                        action.set_state(&param.to_variant());
                    },
                    "provides" => {
                        obj.set_stype(SearchType::Provides);
                        action.set_state(&param.to_variant());
                    },
                    "files" => {
                        obj.set_stype(SearchType::Files);
                        action.set_state(&param.to_variant());
                    },
                    _ => unreachable!()
                }
            }))
            .build();

        // Add cycle search type action
        let cycle_type_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("cycle-type")
            .activate(|group, _, _| {
                if let Some(type_action) = group.lookup_action("set-type") {
                    let state = type_action.state()
                        .expect("Must be a 'Variant'")
                        .get::<String>()
                        .expect("Must be a 'String'");

                    match state.as_str() {
                        "name" => type_action.change_state(&"desc".to_variant()),
                        "desc" => type_action.change_state(&"group".to_variant()),
                        "group" => type_action.change_state(&"deps".to_variant()),
                        "deps" => type_action.change_state(&"optdeps".to_variant()),
                        "optdeps" => type_action.change_state(&"provides".to_variant()),
                        "provides" => type_action.change_state(&"files".to_variant()),
                        "files" => type_action.change_state(&"name".to_variant()),
                        _ => unreachable!()
                    };
                }
            })
            .build();

        // Add reverse cycle search type action
        let reverse_type_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("rev-cycle-type")
            .activate(|group, _, _| {
                if let Some(type_action) = group.lookup_action("set-type") {
                    let state = type_action.state()
                        .expect("Must be a 'Variant'")
                        .get::<String>()
                        .expect("Must be a 'String'");

                    match state.as_str() {
                        "name" => type_action.change_state(&"files".to_variant()),
                        "desc" => type_action.change_state(&"name".to_variant()),
                        "group" => type_action.change_state(&"desc".to_variant()),
                        "deps" => type_action.change_state(&"group".to_variant()),
                        "optdeps" => type_action.change_state(&"deps".to_variant()),
                        "provides" => type_action.change_state(&"optdeps".to_variant()),
                        "files" => type_action.change_state(&"provides".to_variant()),
                        _ => unreachable!()
                    };
                }
            })
            .build();

        // Add reset search params action
        let reset_params_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("reset-params")
            .activate(|group, _, _| {
                if let Some(mode_action) = group.lookup_action("set-mode") {
                    mode_action.change_state(&"all".to_variant());
                }

                if let Some(type_action) = group.lookup_action("set-type") {
                    type_action.change_state(&"name".to_variant());
                }
            })
            .build();

        // Add actions to search action group
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        search_group.add_action(&aur_action);

        search_group.add_action_entries([mode_action, cycle_mode_action, reverse_mode_action, mode_all_action, mode_any_action, mode_exact_action, type_action, cycle_type_action, reverse_type_action, reset_params_action]);

        // Add search type numbered actions
        let nicks: Vec<String> = glib::EnumClass::new::<SearchType>().values().iter()
            .map(|value| value.nick().to_string())
            .collect();

        for nick in nicks {
            let action = gio::ActionEntry::<gio::SimpleActionGroup>::builder(&format!("set-type-{}", nick))
                .activate(move |group, _, _| {
                    if let Some(type_action) = group.lookup_action("set-type") {
                        type_action.change_state(&nick.to_variant());
                    }
                })
                .build();

            search_group.add_action_entries([action]);
        }

        // Store search action group
        self.imp().search_action_group.replace(Some(search_group));
    }

    //-----------------------------------
    // Setup shortcuts
    //-----------------------------------
    fn setup_shortcuts(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();

        // Add include AUR shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>P"),
            Some(gtk::NamedAction::new("search.include-aur"))
        ));

        // Add cycle search mode shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>M"),
            Some(gtk::NamedAction::new("search.cycle-mode"))
        ));

        // Add reverse cycle search mode shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>M"),
            Some(gtk::NamedAction::new("search.rev-cycle-mode"))
        ));

        // Add mode letter shortcuts
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>L"),
            Some(gtk::NamedAction::new("search.set-mode-all"))
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>N"),
            Some(gtk::NamedAction::new("search.set-mode-any"))
        ));

        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>E"),
            Some(gtk::NamedAction::new("search.set-mode-exact"))
        ));

        // Add cycle search type shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>T"),
            Some(gtk::NamedAction::new("search.cycle-type"))
        ));

        // Add reverse cycle search type shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>T"),
            Some(gtk::NamedAction::new("search.rev-cycle-type"))
        ));

        // Add reset search params shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>R"),
            Some(gtk::NamedAction::new("search.reset-params"))
        ));

        // Add search type numbered shortcuts
        let enum_class = glib::EnumClass::new::<SearchType>();

        for (i, v) in enum_class.values().iter().enumerate() {
            controller.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(&format!("<ctrl>{}", i+1)),
                Some(gtk::NamedAction::new(&format!("search.set-type-{}", v.nick()))))
            );
        }

        // Add shortcut controller to search header
        self.add_controller(controller);
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

        controller.connect_key_pressed(clone!(@weak self as header => @default-return glib::Propagation::Proceed, move |controller, _, _, state| {
            if !(state.contains(gdk::ModifierType::ALT_MASK) || state.contains(gdk::ModifierType::CONTROL_MASK))
            {
                if controller.forward(&header.imp().search_text.get()) {
                    header.set_enabled(true);
                }
            }

            glib::Propagation::Proceed
        }));

        widget.add_controller(controller.clone());

        imp.capture_widget.replace(Some(widget));

        imp.capture_controller.replace(controller);
    }
}
