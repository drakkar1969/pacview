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
// ENUM: SearchProp
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchProp")]
pub enum SearchProp {
    Name = 0,
    Desc = 1,
    Group = 2,
    Deps = 3,
    Optdeps = 4,
    Provides = 5,
    Files = 6,
}

impl Default for SearchProp {
    fn default() -> Self {
        SearchProp::Name
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
        pub tag_prop: TemplateChild<SearchTag>,

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
        #[property(get, set, builder(SearchProp::default()))]
        prop: Cell<SearchProp>,

        #[property(get, set)]
        include_aur: Cell<bool>,
        #[property(get, set, default = 150, construct)]
        delay: Cell<u64>,

        #[property(get, set)]
        spinning: Cell<bool>,

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
                            SearchProp::static_type(),
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

            header.activate_action("search.reset-params", None).unwrap();
        });

        // Search mode property notify signal
        self.connect_mode_notify(|header| {
            if let Some((_, value)) = glib::EnumValue::from_value(&header.mode().to_value()) {
                header.imp().tag_mode.set_text(Some(value.nick()));

                header.emit_changed_signal();
            }
        });

        // Search prop property notify signal
        self.connect_prop_notify(|header| {
            if let Some((_, value)) = glib::EnumValue::from_value(&header.prop().to_value()) {
                header.imp().tag_prop.set_text(Some(value.nick()));

                header.emit_changed_signal();
            }
        });

        // Spinning property notify signal
        self.connect_spinning_notify(|header| {
            let imp = header.imp();

            imp.icon_stack.set_visible_child_name(if header.spinning() { "spinner" } else { "icon" });
            imp.spinner.set_spinning(header.spinning());
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
                &self.prop(),
                &self.include_aur()
            ]);
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        // Add include AUR property action
        let aur_action = gio::PropertyAction::new("include-aur", self, "include-aur");

        // Add search mode property action
        let mode_action = gio::PropertyAction::new("set-mode", self, "mode");

        // Add search prop property action
        let prop_action = gio::PropertyAction::new("set-prop", self, "prop");

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

        // Add cycle search prop action
        let cycle_prop_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("cycle-prop")
            .activate(|group, _, _| {
                if let Some(prop_action) = group.lookup_action("set-prop") {
                    let state = prop_action.state()
                        .expect("Must be a 'Variant'")
                        .get::<String>()
                        .expect("Must be a 'String'");

                    match state.as_str() {
                        "name" => prop_action.change_state(&"desc".to_variant()),
                        "desc" => prop_action.change_state(&"group".to_variant()),
                        "group" => prop_action.change_state(&"deps".to_variant()),
                        "deps" => prop_action.change_state(&"optdeps".to_variant()),
                        "optdeps" => prop_action.change_state(&"provides".to_variant()),
                        "provides" => prop_action.change_state(&"files".to_variant()),
                        "files" => prop_action.change_state(&"name".to_variant()),
                        _ => unreachable!()
                    };
                }
            })
            .build();

        // Add reverse cycle search prop action
        let reverse_prop_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("rev-cycle-prop")
            .activate(|group, _, _| {
                if let Some(prop_action) = group.lookup_action("set-prop") {
                    let state = prop_action.state()
                        .expect("Must be a 'Variant'")
                        .get::<String>()
                        .expect("Must be a 'String'");

                    match state.as_str() {
                        "name" => prop_action.change_state(&"files".to_variant()),
                        "desc" => prop_action.change_state(&"name".to_variant()),
                        "group" => prop_action.change_state(&"desc".to_variant()),
                        "deps" => prop_action.change_state(&"group".to_variant()),
                        "optdeps" => prop_action.change_state(&"deps".to_variant()),
                        "provides" => prop_action.change_state(&"optdeps".to_variant()),
                        "files" => prop_action.change_state(&"provides".to_variant()),
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

                if let Some(prop_action) = group.lookup_action("set-prop") {
                    prop_action.change_state(&"name".to_variant());
                }
            })
            .build();

        // Add actions to search action group
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        search_group.add_action(&aur_action);
        search_group.add_action(&mode_action);
        search_group.add_action(&prop_action);

        search_group.add_action_entries([cycle_mode_action, reverse_mode_action, cycle_prop_action, reverse_prop_action, reset_params_action]);
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

        // Add search mode letter shortcuts
        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<ctrl>L"),
            Some(gtk::NamedAction::new("search.set-mode")),
            &"all".to_variant()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<ctrl>N"),
            Some(gtk::NamedAction::new("search.set-mode")),
            &"any".to_variant()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<ctrl>E"),
            Some(gtk::NamedAction::new("search.set-mode")),
            &"exact".to_variant()
        ));

        // Add cycle search prop shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>T"),
            Some(gtk::NamedAction::new("search.cycle-prop"))
        ));

        // Add reverse cycle search prop shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>T"),
            Some(gtk::NamedAction::new("search.rev-cycle-prop"))
        ));

        // Add search prop numbered shortcuts
        let enum_class = glib::EnumClass::new::<SearchProp>();

        for (i, value) in enum_class.values().iter().enumerate() {
            controller.add_shortcut(gtk::Shortcut::with_arguments(
                gtk::ShortcutTrigger::parse_string(&format!("<ctrl>{}", i+1)),
                Some(gtk::NamedAction::new("search.set-prop")),
                &value.nick().to_variant()
            ));
        }

        // Add reset search params shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>R"),
            Some(gtk::NamedAction::new("search.reset-params"))
        ));

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
