use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use core::time::Duration;

use gtk::{glib, gio, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::clone;

use crate::search_tag::SearchTag;
use crate::traits::{EnumValueExt, EnumClassExt};

//------------------------------------------------------------------------------
// ENUM: SearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchMode")]
pub enum SearchMode {
    #[default]
    All,
    Any,
    Exact,
}

impl EnumValueExt for SearchMode {}
impl EnumClassExt for SearchMode {}

//------------------------------------------------------------------------------
// ENUM: SearchProp
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchProp")]
pub enum SearchProp {
    #[default]
    Name,
    NameDesc,
    Group,
    Deps,
    Optdeps,
    Provides,
    Files,
}

impl EnumValueExt for SearchProp {}
impl EnumClassExt for SearchProp {}

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
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) title_widget: TemplateChild<adw::WindowTitle>,

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
        searching: Cell<bool>,

        pub(super) delay_source_id: RefCell<Option<glib::SourceId>>,

        pub(super) action_group: RefCell<gio::SimpleActionGroup>,
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
            klass.bind_template();
            klass.set_layout_manager_type::<gtk::BoxLayout>();
            klass.set_css_name("search-header");
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
            let obj = self.obj();

            if aur_error {
                obj.add_css_class("error");
            } else {
                obj.remove_css_class("error");
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

        // Bind searching property to widgets
        self.bind_property("searching", &imp.icon_stack.get(), "visible-child-name")
            .transform_to(|_, searching: bool| if searching { Some("spinner") } else { Some("icon") })
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
            header.imp().tag_mode.set_text(Some(header.mode().nick()));

            header.emit_changed_signal();
        });

        // Search prop property notify signal
        self.connect_prop_notify(|header| {
            header.imp().tag_prop.set_text(Some(header.prop().nick()));

            header.emit_changed_signal();
        });

        // Search text changed signal
        imp.search_text.connect_changed(clone!(
            #[weak(rename_to = header)]
            self,
            #[weak]
            imp,
            move |search_text| {
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
                        clone!(
                            #[weak]
                            imp,
                            move || {
                                header.emit_changed_signal();

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
            #[weak(rename_to = header)]
            self,
            move |search_text| {
                if !search_text.text().is_empty() {
                    header.emit_aur_search_signal();
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
            .activate(|group: &gio::SimpleActionGroup, _, _| {
                group.activate_action("set-mode", Some(&SearchMode::All.to_variant()));
                group.activate_action("set-prop", Some(&SearchProp::Name.to_variant()));
            })
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
                let header = widget
                    .downcast_ref::<SearchHeader>()
                    .expect("Could not downcast to 'SearchHeader'");

                let action_group = header.imp().action_group.borrow();

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
                let header = widget
                    .downcast_ref::<SearchHeader>()
                    .expect("Could not downcast to 'SearchHeader'");

                let action_group = header.imp().action_group.borrow();

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
            &SearchMode::All.to_variant()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<ctrl>N"),
            Some(gtk::NamedAction::new("search.set-mode")),
            &SearchMode::Any.to_variant()
        ));

        controller.add_shortcut(gtk::Shortcut::with_arguments(
            gtk::ShortcutTrigger::parse_string("<ctrl>E"),
            Some(gtk::NamedAction::new("search.set-mode")),
            &SearchMode::Exact.to_variant()
        ));

        // Add cycle search prop shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>T"),
            Some(gtk::CallbackAction::new(|widget, _| {
                let header = widget
                    .downcast_ref::<SearchHeader>()
                    .expect("Could not downcast to 'SearchHeader'");

                let action_group = header.imp().action_group.borrow();

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
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>T"),
            Some(gtk::CallbackAction::new(|widget, _| {
                let header = widget
                    .downcast_ref::<SearchHeader>()
                    .expect("Could not downcast to 'SearchHeader'");

                let action_group = header.imp().action_group.borrow();

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

        // Add shortcut controller to search header
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
                #[weak(rename_to = header)] self,
                #[upgrade_or] glib::Propagation::Proceed,
                move |controller, _, _, state| {
                    if !(state.contains(gdk::ModifierType::ALT_MASK) ||
                        state.contains(gdk::ModifierType::CONTROL_MASK)) &&
                        controller.forward(&header.imp().search_text.get())
                    {
                        header.set_enabled(true);
                    }

                    glib::Propagation::Proceed
                }
            ));

            widget.add_controller(controller);

            imp.has_capture_widget.set(true);
        }
    }
}

impl Default for SearchHeader {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
