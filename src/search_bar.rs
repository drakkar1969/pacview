use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::sync::OnceLock;
use core::time::Duration;

use gtk::{glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::clone;

use strum::{FromRepr, EnumIter, IntoEnumIterator};

use crate::search_tag::SearchTag;
use crate::enum_traits::EnumExt;

//------------------------------------------------------------------------------
// ENUM: SearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, FromRepr, EnumIter)]
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
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, FromRepr, EnumIter)]
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
        text: PhantomData<glib::GString>,
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

            //---------------------------------------
            // Add class actions
            //---------------------------------------
            // Search mode property action
            klass.install_property_action("search.set-mode", "mode");

            // Search prop property action
            klass.install_property_action("search.set-prop", "prop");

            // Reset search params action
            klass.install_action("search.reset-params", None, |bar, _, _| {
                bar.activate_action("search.set-mode", Some(&bar.default_mode().nick_variant()))
                    .unwrap();
                bar.activate_action("search.set-prop", Some(&bar.default_prop().nick_variant()))
                    .unwrap();
            });

            //---------------------------------------
            // Add class key bindings
            //---------------------------------------
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
            klass.add_binding(gdk::Key::L, gdk::ModifierType::CONTROL_MASK, |bar| {
                bar.activate_action("search.set-mode", Some(&SearchMode::All.nick_variant())).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(gdk::Key::N, gdk::ModifierType::CONTROL_MASK, |bar| {
                bar.activate_action("search.set-mode", Some(&SearchMode::Any.nick_variant())).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(gdk::Key::E, gdk::ModifierType::CONTROL_MASK, |bar| {
                bar.activate_action("search.set-mode", Some(&SearchMode::Exact.nick_variant()))
                    .unwrap();

                    glib::Propagation::Stop
            });

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
            for (i, prop) in SearchProp::iter().enumerate() {
                let key = gdk::Key::from_name((i+1).to_string()).unwrap();
                
                klass.add_binding(key, gdk::ModifierType::CONTROL_MASK, move |bar| {
                    bar.activate_action("search.set-prop", Some(&prop.nick_variant())).unwrap();

                    glib::Propagation::Stop
                });
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
        // Signals
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

            obj.setup_signals();
        }
    }

    impl WidgetImpl for SearchBar {}
    impl BinImpl for SearchBar {}

    impl SearchBar {
        //---------------------------------------
        // Property getter
        //---------------------------------------
        fn text(&self) -> glib::GString {
            self.search_text.text()
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
    // Public set AUR status function
    //---------------------------------------
    pub fn set_aur_status(&self, status: Result<(), String>) {
        let imp = self.imp();

        if let Err(mut error) = status {
            self.add_css_class("error");

            error.pop();

            imp.error_label.set_label(&error);
            imp.error_button.set_visible(true);
        } else {
            self.remove_css_class("error");

            imp.error_label.set_label("");
            imp.error_button.set_visible(false);
        }
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search enabled property notify signal
        self.connect_enabled_notify(|bar| {
            let imp = bar.imp();

            let enabled = bar.enabled();

            imp.revealer.set_reveal_child(enabled);

            if enabled {
                imp.search_text.grab_focus_without_selecting();
            } else {
                imp.search_text.set_text("");
            }
        });

        // Searching property notify signal
        self.connect_searching_notify(|bar| {
            bar.imp().icon_stack.set_visible_child_name(
                if bar.searching() { "spinner" } else { "icon" }
            );
        });

        // Search mode property notify signal
        self.connect_mode_notify(|bar| {
            let imp = bar.imp();

            imp.tag_mode.set_text(bar.mode().nick());

            if !imp.search_text.text().is_empty() {
                bar.emit_by_name::<()>("changed", &[]);
            }
        });

        // Search prop property notify signal
        self.connect_prop_notify(|bar| {
            let imp = bar.imp();

            imp.tag_prop.set_text(bar.prop().nick());

            if !imp.search_text.text().is_empty() {
                bar.emit_by_name::<()>("changed", &[]);
            }
        });

        // Search text changed signal
        imp.search_text.connect_changed(clone!(
            #[weak(rename_to = bar)] self,
            #[weak] imp,
            move |search_text| {
                let text = search_text.text();

                imp.clear_button.set_visible(!text.is_empty());

                // Remove delay timer if present
                if let Some(delay_id) = imp.delay_source_id.take() {
                    delay_id.remove();
                }

                if text.is_empty() {
                    bar.set_aur_status(Ok(()));

                    bar.emit_by_name::<()>("changed", &[]);
                    bar.emit_by_name::<()>("aur-search", &[]);
                } else {
                    // Start delay timer
                    let delay_id = glib::timeout_add_local_once(
                        Duration::from_millis(bar.delay()),
                        clone!(
                            #[weak] imp,
                            move || {
                                bar.emit_by_name::<()>("changed", &[]);

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
                    bar.emit_by_name::<()>("aur-search", &[]);
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
        glib::Object::builder().build()
    }
}
