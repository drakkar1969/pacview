use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use core::time::Duration;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::clone;

use strum::{EnumString, FromRepr};

use crate::search_tag::SearchTag;
use crate::traits::{EnumValueExt, EnumClassExt};

//------------------------------------------------------------------------------
// ENUM: SearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, EnumString, FromRepr)]
#[strum(serialize_all = "kebab-case")]
#[repr(u32)]
#[enum_type(name = "SearchMode")]
pub enum SearchMode {
    #[default]
    #[enum_value(name = "Match all terms")]
    All,
    #[enum_value(name = "Match any term")]
    Any,
    #[enum_value(name = "Exact match")]
    Exact,
}

impl EnumValueExt for SearchMode {}
impl EnumClassExt for SearchMode {}

//------------------------------------------------------------------------------
// ENUM: SearchProp
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, EnumString, FromRepr)]
#[strum(serialize_all = "kebab-case")]
#[repr(u32)]
#[enum_type(name = "SearchProp")]
pub enum SearchProp {
    #[default]
    #[enum_value(name = "Name")]
    Name,
    #[enum_value(name = "Name or Description")]
    NameDesc,
    #[enum_value(name = "Group")]
    Group,
    #[enum_value(name = "Dependencies")]
    Deps,
    #[enum_value(name = "Optional Dependencies")]
    Optdeps,
    #[enum_value(name = "Provides")]
    Provides,
    #[enum_value(name = "Files")]
    Files,
}

impl EnumValueExt for SearchProp {}
impl EnumClassExt for SearchProp {}

//------------------------------------------------------------------------------
// MODULE: SearchBar
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::SearchBar)]
    #[template(resource = "/com/github/PacView/ui/search_bar.ui")]
    pub struct SearchBar {
        #[template_child]
        pub(super) revealer: TemplateChild<gtk::Revealer>,

        #[template_child]
        pub(super) icon_stack: TemplateChild<gtk::Stack>,

        #[template_child]
        pub(super) tag_mode: TemplateChild<SearchTag>,
        #[template_child]
        pub(super) tag_prop: TemplateChild<SearchTag>,

        #[template_child]
        pub(super) search_text: TemplateChild<gtk::Text>,

        #[template_child]
        pub(super) clear_button: TemplateChild<gtk::Button>,

        pub(super) has_capture_widget: Cell<bool>,

        #[property(get, set)]
        enabled: Cell<bool>,

        #[property(get, set, builder(SearchMode::default()))]
        mode: Cell<SearchMode>,
        #[property(get, set, builder(SearchProp::default()))]
        prop: Cell<SearchProp>,
        #[property(get, set, builder(SearchMode::default()))]
        default_mode: Cell<SearchMode>,
        #[property(get, set, builder(SearchProp::default()))]
        default_prop: Cell<SearchProp>,

        #[property(get, set, default = 150, construct)]
        delay: Cell<u64>,

        #[property(get, set)]
        searching: Cell<bool>,

        pub(super) delay_source_id: RefCell<Option<glib::SourceId>>,

        pub(super) action_group: RefCell<gio::SimpleActionGroup>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SearchBar {
        const NAME: &'static str = "SearchBar";
        type Type = super::SearchBar;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("search-bar");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SearchBar {
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
                            SearchProp::static_type()
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
    }

    impl WidgetImpl for SearchBar {}
    impl BinImpl for SearchBar {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SearchBar
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchBar(ObjectSubclass<imp::SearchBar>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SearchBar {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Public set AUR error function
    //-----------------------------------
    pub fn set_aur_error(&self, aur_error: Option<String>) {
        if let Some(error_msg) = aur_error {
            self.add_css_class("error");
            self.set_tooltip_text(Some(&format!("Error: {}", error_msg)));
        } else {
            self.remove_css_class("error");
            self.set_tooltip_text(None);
        }
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind enabled property to revealer
        self.bind_property("enabled", &imp.revealer.get(), "reveal-child")
            .sync_create()
            .build();

        // Bind searching property to icon stack
        self.bind_property("searching", &imp.icon_stack.get(), "visible-child-name")
            .transform_to(|_, searching: bool| if searching { Some("spinner") } else { Some("icon") })
            .sync_create()
            .build();

        // Bind search text to clear button visibility
        imp.search_text.bind_property("text", &imp.clear_button.get(), "visible")
            .transform_to(|_, text: &str| Some(!text.is_empty()))
            .sync_create()
            .build();
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search enabled property notify signal
        self.connect_enabled_notify(|bar| {
            let imp = bar.imp();

            if bar.enabled() {
                imp.search_text.grab_focus_without_selecting();
            } else {
                imp.search_text.set_text("");
            }

            bar.emit_by_name::<()>("enabled", &[&bar.enabled()]);
        });

        // Search mode property notify signal
        self.connect_mode_notify(|bar| {
            bar.imp().tag_mode.set_text(Some(bar.mode().nick()));

            bar.emit_changed_signal();
        });

        // Search prop property notify signal
        self.connect_prop_notify(|bar| {
            bar.imp().tag_prop.set_text(Some(bar.prop().nick()));

            bar.emit_changed_signal();
        });

        // Search text changed signal
        imp.search_text.connect_changed(clone!(
            #[weak(rename_to = bar)]
            self,
            #[weak]
            imp,
            move |search_text| {
                // Remove delay timer if present
                if let Some(delay_id) = imp.delay_source_id.take() {
                    delay_id.remove();
                }

                if search_text.text().is_empty() {
                    bar.set_aur_error(None::<String>);

                    bar.emit_changed_signal();
                    bar.emit_aur_search_signal();
                } else {
                    // Start delay timer
                    let delay_id = glib::timeout_add_local_once(
                        Duration::from_millis(bar.delay()),
                        clone!(
                            #[weak]
                            imp,
                            move || {
                                bar.emit_changed_signal();

                                imp.delay_source_id.take();
                            }
                        )
                    );

                    imp.delay_source_id.replace(Some(delay_id));
                }
            }
        ));

        // Search text activate signal
        imp.search_text.connect_activate(clone!(
            #[weak(rename_to = bar)]
            self,
            move |search_text| {
                if !search_text.text().is_empty() {
                    bar.emit_aur_search_signal();
                }
            }
        ));

        // Clear button clicked signal
        imp.clear_button.connect_clicked(clone!(
            #[weak]
            imp,
            move |_| {
                imp.search_text.set_text("");
            }
        ));
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
                &self.prop()
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

        // Add reset search params action
        let reset_params_action = gio::ActionEntry::builder("reset-params")
            .activate(clone!(
                #[weak(rename_to = bar)] self,
                move |group: &gio::SimpleActionGroup, _, _| {
                    group.activate_action("set-mode", Some(&bar.default_mode().variant_nick()));
                    group.activate_action("set-prop", Some(&bar.default_prop().variant_nick()));
                }
            ))
            .build();

        // Add actions to search action group
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        search_group.add_action(&mode_action);
        search_group.add_action(&prop_action);

        search_group.add_action_entries([reset_params_action]);

        // Store search action group
        self.imp().action_group.replace(search_group);
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
            Some(gtk::CallbackAction::new(|widget, _| {
                let bar = widget
                    .downcast_ref::<SearchBar>()
                    .expect("Could not downcast to 'SearchBar'");

                let action_group = bar.imp().action_group.borrow();

                let state = action_group.action_state("set-mode")
                    .expect("Could not retrieve Variant")
                    .get::<String>()
                    .expect("Could not retrieve String from variant");

                let new_state = SearchMode::next_nick(&state);

                action_group.activate_action("set-mode", Some(&new_state.to_variant()));

                glib::Propagation::Proceed
            }))
        ));

        // Add reverse cycle search mode shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>M"),
            Some(gtk::CallbackAction::new(|widget, _| {
                let bar = widget
                    .downcast_ref::<SearchBar>()
                    .expect("Could not downcast to 'SearchBar'");

                let action_group = bar.imp().action_group.borrow();

                let state = action_group.action_state("set-mode")
                    .expect("Could not retrieve Variant")
                    .get::<String>()
                    .expect("Could not retrieve String from variant");

                let new_state = SearchMode::previous_nick(&state);

                action_group.activate_action("set-mode", Some(&new_state.to_variant()));

                glib::Propagation::Proceed
            }))
        ));

        // Add search mode letter shortcuts
        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<ctrl>L"),
            Some(gtk::NamedAction::new("search.set-mode")),
            &SearchMode::All.variant_nick()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<ctrl>N"),
            Some(gtk::NamedAction::new("search.set-mode")),
            &SearchMode::Any.variant_nick()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<ctrl>E"),
            Some(gtk::NamedAction::new("search.set-mode")),
            &SearchMode::Exact.variant_nick()
        ));

        // Add cycle search prop shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>P"),
            Some(gtk::CallbackAction::new(|widget, _| {
                let bar = widget
                    .downcast_ref::<SearchBar>()
                    .expect("Could not downcast to 'SearchBar'");

                let action_group = bar.imp().action_group.borrow();

                let state = action_group.action_state("set-prop")
                    .expect("Could not retrieve Variant")
                    .get::<String>()
                    .expect("Could not retrieve String from variant");

                let new_state = SearchProp::next_nick(&state);

                action_group.activate_action("set-prop", Some(&new_state.to_variant()));

                glib::Propagation::Proceed
            }))
        ));

        // Add reverse cycle search prop shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>P"),
            Some(gtk::CallbackAction::new(|widget, _| {
                let bar = widget
                    .downcast_ref::<SearchBar>()
                    .expect("Could not downcast to 'SearchBar'");

                let action_group = bar.imp().action_group.borrow();

                let state = action_group.action_state("set-prop")
                    .expect("Could not retrieve Variant")
                    .get::<String>()
                    .expect("Could not retrieve String from variant");

                let new_state = SearchProp::previous_nick(&state);

                action_group.activate_action("set-prop", Some(&new_state.to_variant()));

                glib::Propagation::Proceed
            }))
        ));

        // Add search prop numbered shortcuts
        let enum_class = SearchProp::enum_class();

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

        // Add shortcut controller to search bar
        self.add_controller(controller);
    }

    //-----------------------------------
    // Public set capture widget function
    //-----------------------------------
    pub fn set_key_capture_widget(&self, widget: gtk::Widget) {
        let imp = self.imp();

        if !imp.has_capture_widget.get() {
            let controller = gtk::EventControllerKey::new();

            controller.connect_key_pressed(clone!(
                #[weak(rename_to = bar)] self,
                #[upgrade_or] glib::Propagation::Proceed,
                move |controller, _, _, state| {
                    if !(bar.enabled() || state.contains(gdk::ModifierType::ALT_MASK) ||
                        state.contains(gdk::ModifierType::CONTROL_MASK)) &&
                        controller.forward(&bar.imp().search_text.get())
                    {
                        bar.set_enabled(true);
                    }

                    glib::Propagation::Proceed
                }
            ));

            widget.add_controller(controller);

            imp.has_capture_widget.set(true);
        }
    }
}

impl Default for SearchBar {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
