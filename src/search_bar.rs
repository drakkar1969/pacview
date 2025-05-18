use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use core::time::Duration;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::clone;

use strum::{EnumString, FromRepr, EnumIter, IntoEnumIterator};

use crate::search_tag::SearchTag;
use crate::enum_traits::EnumExt;

//------------------------------------------------------------------------------
// ENUM: SearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, EnumString, FromRepr, EnumIter)]
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

impl EnumExt for SearchMode {}

//------------------------------------------------------------------------------
// ENUM: SearchProp
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, EnumString, FromRepr, EnumIter)]
#[strum(serialize_all = "kebab-case")]
#[repr(u32)]
#[enum_type(name = "SearchProp")]
pub enum SearchProp {
    #[default]
    #[enum_value(name = "Name")]
    Name,
    #[enum_value(name = "Name or Description")]
    NameDesc,
    #[enum_value(name = "Groups")]
    Groups,
    #[enum_value(name = "Dependencies")]
    Deps,
    #[enum_value(name = "Optional Dependencies")]
    Optdeps,
    #[enum_value(name = "Provides")]
    Provides,
    #[enum_value(name = "Files")]
    Files,
}

impl EnumExt for SearchProp {}

//------------------------------------------------------------------------------
// MODULE: SearchBar
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
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

        #[template_child]
        pub(super) error_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) error_label: TemplateChild<gtk::Label>,

        pub(super) has_capture_widget: Cell<bool>,

        #[property(get, set)]
        enabled: Cell<bool>,

        #[property(get = Self::text)]
        _text: RefCell<String>,
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
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SearchBar {
        const NAME: &'static str = "SearchBar";
        type Type = super::SearchBar;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("searchbar");

            // Cycle search mode key binding
            klass.add_binding(gdk::Key::M, gdk::ModifierType::CONTROL_MASK, |bar| {
                let new_mode = SearchMode::iter().cycle()
                    .skip_while(|&mode| mode != bar.mode())
                    .nth(1)
                    .expect("Failed to get 'SearchMode'");

                bar.activate_action("search.set-mode", Some(&new_mode.nick_variant())).unwrap();

                glib::Propagation::Stop
            });

            // Reverse cycle search mode key binding
            klass.add_binding(gdk::Key::M, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK, |bar| {
                let new_mode = SearchMode::iter().rev().cycle()
                    .skip_while(|&mode| mode != bar.mode())
                    .nth(1)
                    .expect("Failed to get 'SearchMode'");

                bar.activate_action("search.set-mode", Some(&new_mode.nick_variant())).unwrap();

                glib::Propagation::Stop
            });

            // Search mode letter shortcuts
            klass.add_shortcut(&gtk::Shortcut::with_arguments(
                gtk::ShortcutTrigger::parse_string("<ctrl>L"),
                Some(gtk::NamedAction::new("search.set-mode")),
                &SearchMode::All.nick_variant()
            ));

            klass.add_shortcut(&gtk::Shortcut::with_arguments(
                gtk::ShortcutTrigger::parse_string("<ctrl>N"),
                Some(gtk::NamedAction::new("search.set-mode")),
                &SearchMode::Any.nick_variant()
            ));

            klass.add_shortcut(&gtk::Shortcut::with_arguments(
                gtk::ShortcutTrigger::parse_string("<ctrl>E"),
                Some(gtk::NamedAction::new("search.set-mode")),
                &SearchMode::Exact.nick_variant()
            ));

            // Cycle search prop key binding
            klass.add_binding(gdk::Key::P, gdk::ModifierType::CONTROL_MASK, |bar| {
                let new_prop = SearchProp::iter().cycle()
                    .skip_while(|&prop| prop != bar.prop())
                    .nth(1)
                    .expect("Failed to get 'SearchProp'");

                bar.activate_action("search.set-prop", Some(&new_prop.nick_variant())).unwrap();

                glib::Propagation::Stop
            });

            // Reverse cycle search prop key binding
            klass.add_binding(gdk::Key::P, gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK, |bar| {
                let new_prop = SearchProp::iter().rev().cycle()
                    .skip_while(|&prop| prop != bar.prop())
                    .nth(1)
                    .expect("Failed to get 'SearchProp'");

                bar.activate_action("search.set-prop", Some(&new_prop.nick_variant())).unwrap();

                glib::Propagation::Stop
            });

            // Search prop numbered shortcuts
            for (i, value) in SearchProp::iter().enumerate() {
                klass.add_shortcut(&gtk::Shortcut::with_arguments(
                    gtk::ShortcutTrigger::parse_string(&format!("<ctrl>{}", i+1)),
                    Some(gtk::NamedAction::new("search.set-prop")),
                    &value.nick_variant()
                ));
            }

            // Reset search params key binding
            klass.add_binding_action(gdk::Key::R, gdk::ModifierType::CONTROL_MASK, "search.reset-params");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SearchBar {
        //---------------------------------------
        // Custom signals
        //---------------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("changed")
                        .build(),
                    Signal::builder("aur-search")
                        .build(),
                ]
            })
        }

        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_signals();
            obj.setup_actions();
        }
    }

    impl WidgetImpl for SearchBar {}
    impl BinImpl for SearchBar {}

    impl SearchBar {
        fn text(&self) -> String {
            self.search_text.text().to_string()
        }
    }
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
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //---------------------------------------
    // Public set AUR error function
    //---------------------------------------
    pub fn set_aur_error(&self, aur_error: Option<String>) {
        let imp = self.imp();

        if let Some(mut error_msg) = aur_error {
            self.add_css_class("error");

            error_msg.pop();

            imp.error_label.set_label(&error_msg);
            imp.error_button.set_visible(true);
        } else {
            self.remove_css_class("error");

            imp.error_label.set_label("");
            imp.error_button.set_visible(false);
        }
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
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

    //---------------------------------------
    // Setup signals
    //---------------------------------------
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
        });

        // Search mode property notify signal
        self.connect_mode_notify(|bar| {
            let imp = bar.imp();

            imp.tag_mode.set_text(Some(bar.mode().nick()));

            if !imp.search_text.text().is_empty() {
                bar.emit_changed_signal();
            }
        });

        // Search prop property notify signal
        self.connect_prop_notify(|bar| {
            let imp = bar.imp();

            imp.tag_prop.set_text(Some(bar.prop().nick()));

            if !imp.search_text.text().is_empty() {
                bar.emit_changed_signal();
            }
        });

        // Search text changed signal
        imp.search_text.connect_changed(clone!(
            #[weak(rename_to = bar)] self,
            #[weak] imp,
            move |search_text| {
                // Remove delay timer if present
                if let Some(delay_id) = imp.delay_source_id.take() {
                    delay_id.remove();
                }

                if search_text.text().is_empty() {
                    bar.set_aur_error(None);

                    bar.emit_changed_signal();
                    bar.emit_aur_search_signal();
                } else {
                    // Start delay timer
                    let delay_id = glib::timeout_add_local_once(
                        Duration::from_millis(bar.delay()),
                        clone!(
                            #[weak] imp,
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
            #[weak(rename_to = bar)] self,
            move |search_text| {
                if !search_text.text().is_empty() {
                    bar.emit_aur_search_signal();
                }
            }
        ));

        // Clear button clicked signal
        imp.clear_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                imp.search_text.set_text("");

                if !imp.search_text.has_focus() {
                    imp.search_text.grab_focus_without_selecting();
                }
            }
        ));
    }

    //---------------------------------------
    // Signal emit helper functions
    //---------------------------------------
    fn emit_changed_signal(&self) {
        self.emit_by_name::<()>("changed", &[]);
    }

    fn emit_aur_search_signal(&self) {
        self.emit_by_name::<()>("aur-search", &[]);
    }

    //---------------------------------------
    // Setup actions
    //---------------------------------------
    fn setup_actions(&self) {
        // Search mode property action
        let mode_action = gio::PropertyAction::new("set-mode", self, "mode");

        // Search prop property action
        let prop_action = gio::PropertyAction::new("set-prop", self, "prop");

        // Reset search params action
        let reset_params_action = gio::ActionEntry::builder("reset-params")
            .activate(clone!(
                #[weak(rename_to = bar)] self,
                move |group: &gio::SimpleActionGroup, _, _| {
                    group.activate_action("set-mode", Some(&bar.default_mode().nick_variant()));
                    group.activate_action("set-prop", Some(&bar.default_prop().nick_variant()));
                }
            ))
            .build();

        // Add actions to search action group
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        search_group.add_action(&mode_action);
        search_group.add_action(&prop_action);

        search_group.add_action_entries([reset_params_action]);
    }

    //---------------------------------------
    // Public set capture widget function
    //---------------------------------------
    pub fn set_key_capture_widget(&self, widget: &gtk::Widget) {
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
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        Self::new()
    }
}
