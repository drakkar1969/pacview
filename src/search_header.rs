use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use core::time::Duration;

use gtk::{glib, gio, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::clone;

use crate::search_tag::SearchTag;

//------------------------------------------------------------------------------
// ENUM: SearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchMode")]
pub enum SearchMode {
    #[default]
    All = 0,
    Any = 1,
    Exact = 2,
}

impl SearchMode {
    pub fn modes() -> Vec<String> {
        let mode_enum = glib::EnumClass::new::<Self>();

        mode_enum.values()
            .iter()
            .map(|v| {v.nick().to_string()})
            .collect::<Vec<String>>()
    }
}

//------------------------------------------------------------------------------
// ENUM: SearchProp
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchProp")]
pub enum SearchProp {
    #[default]
    Name = 0,
    NameDesc = 1,
    Group = 2,
    Deps = 3,
    Optdeps = 4,
    Provides = 5,
    Files = 6,
}

impl SearchProp {
    pub fn props() -> Vec<String> {
        let prop_enum = glib::EnumClass::new::<Self>();

        prop_enum.values()
            .iter()
            .map(|v| {v.nick().to_string()})
            .collect::<Vec<String>>()
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

        #[property(get, set = Self::set_aur_error)]
        aur_error: Cell<bool>,

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
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("enabled")
                        .param_types([bool::static_type()])
                        .build(),
                    Signal::builder("changed")
                        .param_types([
                            String::static_type(),
                            SearchMode::static_type(),
                            SearchProp::static_type()
                        ])
                        .build(),
                    Signal::builder("aur-search")
                        .param_types([
                            String::static_type(),
                            SearchMode::static_type(),
                            SearchProp::static_type(),
                            bool::static_type()
                        ])
                        .build(),
                ]
            })
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

    impl SearchHeader {
        //-----------------------------------
        // Custom AUR error property setter
        //-----------------------------------
        fn set_aur_error(&self, aur_error: bool) {
            if aur_error {
                self.search_text.add_css_class("error");
            } else {
                self.search_text.remove_css_class("error");
            }

            self.aur_error.set(aur_error);
        }
    }
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
            .transform_to(|_, text: &str| Some(!text.is_empty()))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind spinning property to widgets
        self.bind_property("spinning", &imp.icon_stack.get(), "visible-child-name")
            .transform_to(|_, spinning: bool| if spinning { Some("spinner") } else { Some("icon") })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        self.bind_property("spinning", &imp.spinner.get(), "spinning")
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

        // Search text changed signal
        imp.search_text.connect_changed(clone!(@weak self as header, @weak imp => move |search_text| {
            // Remove delay timer if present
            if let Some(delay_id) = imp.delay_source_id.take() {
                delay_id.remove();
            }

            if search_text.text().is_empty() {
                header.set_aur_error(false);

                header.emit_changed_signal();
                header.emit_aur_search_signal();
            } else {
                // Start delay timer
                let delay_id = glib::timeout_add_local_once(
                    Duration::from_millis(header.delay()),
                    clone!(@weak imp => move || {
                        header.emit_changed_signal();

                        imp.delay_source_id.take();
                    })
                );

                imp.delay_source_id.replace(Some(delay_id));
            }
        }));

        // Search text activate signal
        imp.search_text.connect_activate(clone!(@weak self as header => move |search_text| {
            let text = search_text.text();

            if text.split_whitespace().any(|t| t.len() < 4) {
                header.set_aur_error(true);
            } else {
                header.set_aur_error(false);
            }

            header.emit_aur_search_signal();
        }));

        // Clear button clicked signal
        imp.clear_button.connect_clicked(clone!(@weak imp => move |_| {
            imp.search_text.set_text("");
        }));
    }

    //-----------------------------------
    // Signal emit helper functions
    //-----------------------------------
    fn emit_changed_signal(&self) {
        let imp = self.imp();

        self.emit_by_name::<()>("changed",
            &[
                &imp.search_text.text(),
                &self.mode(),
                &self.prop()
            ]);
    }

    fn emit_aur_search_signal(&self) {
        let imp = self.imp();

        self.emit_by_name::<()>("aur-search",
            &[
                &imp.search_text.text(),
                &self.mode(),
                &self.prop(),
                &self.aur_error()
            ]);
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        // Add search mode property action
        let mode_action = gio::PropertyAction::new("set-mode", self, "mode");

        // Add search prop property action
        let prop_action = gio::PropertyAction::new("set-prop", self, "prop");

        // Add cycle search mode action
        let cycle_mode_action = gio::ActionEntry::builder("cycle-mode")
            .activate(|group: &gio::SimpleActionGroup, _, _| {
                let state = group.action_state("set-mode")
                    .expect("Could not retrieve Variant")
                    .get::<String>()
                    .expect("Could not retrieve String from variant");

                let modes = SearchMode::modes();

                let new_state = modes.iter().position(|s| s == &state)
                    .and_then(|i| i.checked_add(1))
                    .and_then(|i| modes.get(i))
                    .unwrap_or(&modes[0]);

                group.activate_action("set-mode", Some(&new_state.to_variant()));
            })
            .build();

        // Add reverse cycle search mode action
        let reverse_mode_action = gio::ActionEntry::builder("rev-cycle-mode")
            .activate(|group: &gio::SimpleActionGroup, _, _| {
                let state = group.action_state("set-mode")
                    .expect("Could not retrieve Variant")
                    .get::<String>()
                    .expect("Could not retrieve String from variant");

                let modes = SearchMode::modes();

                let new_state = modes.iter().position(|s| s == &state)
                    .and_then(|i| i.checked_sub(1))
                    .and_then(|i| modes.get(i))
                    .unwrap_or(modes.last().unwrap());

                group.activate_action("set-mode", Some(&new_state.to_variant()));
                })
            .build();

        // Add cycle search prop action
        let cycle_prop_action = gio::ActionEntry::builder("cycle-prop")
            .activate(|group: &gio::SimpleActionGroup, _, _| {
                let state = group.action_state("set-prop")
                    .expect("Could not retrieve Variant")
                    .get::<String>()
                    .expect("Could not retrieve String from variant");

                let props = SearchProp::props();

                let new_state = props.iter().position(|s| s == &state)
                    .and_then(|i| i.checked_add(1))
                    .and_then(|i| props.get(i))
                    .unwrap_or(&props[0]);

                group.activate_action("set-prop", Some(&new_state.to_variant()));
            })
            .build();

        // Add reverse cycle search prop action
        let reverse_prop_action = gio::ActionEntry::builder("rev-cycle-prop")
            .activate(|group: &gio::SimpleActionGroup, _, _| {
                let state = group.action_state("set-prop")
                    .expect("Could not retrieve Variant")
                    .get::<String>()
                    .expect("Could not retrieve String from variant");

                let props = SearchProp::props();

                let new_state = props.iter().position(|s| s == &state)
                    .and_then(|i| i.checked_sub(1))
                    .and_then(|i| props.get(i))
                    .unwrap_or(props.last().unwrap());

                group.activate_action("set-prop", Some(&new_state.to_variant()));
            })
            .build();

        // Add reset search params action
        let reset_params_action = gio::ActionEntry::builder("reset-params")
            .activate(|group: &gio::SimpleActionGroup, _, _| {
                group.activate_action("set-mode", Some(&"all".to_variant()));
                group.activate_action("set-prop", Some(&"name".to_variant()));
            })
            .build();

        // Add actions to search action group
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

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
