use std::sync::LazyLock;
use std::fmt::Write as _;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use gdk::{Key, ModifierType};

use regex::Regex;
use rayon::prelude::*;

use crate::vars::Pacman;
use crate::log_object::{LogLine, LogObject};

//------------------------------------------------------------------------------
// MODULE: LogWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/log_window.ui")]
    pub struct LogWindow {
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) package_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::NoSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) package_filter: TemplateChild<gtk::CustomFilter>,

        #[template_child]
        pub(super) footer_label: TemplateChild<gtk::Label>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for LogWindow {
        const NAME: &'static str = "LogWindow";
        type Type = super::LogWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            LogObject::ensure_type();

            klass.bind_template();

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LogWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_widgets();
        }
    }

    impl WidgetImpl for LogWindow {}
    impl WindowImpl for LogWindow {}
    impl AdwWindowImpl for LogWindow {}

    impl LogWindow {
        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");

            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if !imp.search_entry.has_focus() {
                    imp.search_entry.grab_focus();
                }

                glib::Propagation::Stop
            });

            // Filter package events key binding
            klass.add_binding(Key::P, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                imp.package_button.set_active(!imp.package_button.is_active());

                glib::Propagation::Stop
            });

            // Copy key binding
            klass.add_binding(Key::C, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.copy_button.is_sensitive() {
                    imp.copy_button.emit_clicked();
                }

                glib::Propagation::Stop
            });
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: LogWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct LogWindow(ObjectSubclass<imp::LogWindow>)
    @extends adw::Window, gtk::Window, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl LogWindow {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search entry search started signal
        imp.search_entry.connect_search_started(|entry| {
            if !entry.has_focus() {
                entry.grab_focus();
            }
        });

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.search_filter.set_search(Some(&entry.text()));
            }
        ));

        // Package button toggled signal
        imp.package_button.connect_toggled(clone!(
            #[weak] imp,
            move |_| {
                imp.package_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                let mut output = String::from("## Log Messages\n|Date|Time|Category|Message|\n|---|---|---|---|\n");

                for log in window.imp().selection.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|item| item.downcast::<LogObject>().ok()) {
                        let _ = writeln!(output, "|{date}|{time}|{category}|{message}|",
                            date=log.date(),
                            time=log.time(),
                            category=log.category(),
                            message=log.message()
                        );
                    }

                window.clipboard().set_text(&output);
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.stack.set_visible_child_name(if n_items == 0 { "empty" } else { "view" });

                imp.footer_label.set_label(&format!("{n_items} line{}", if n_items == 1 { "" } else { "s" }));

                imp.copy_button.set_sensitive(n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

        // Set package filter function
        imp.package_filter.set_filter_func(clone!(
            #[weak] imp,
            #[upgrade_or] false,
            move |item| {
                if imp.package_button.is_active() {
                    let msg = item
                        .downcast_ref::<LogObject>()
                        .expect("Failed to downcast to 'LogObject'")
                        .message();

                    msg.starts_with("installed ") || msg.starts_with("removed ") || msg.starts_with("upgraded ") || msg.starts_with("downgraded ")
                } else {
                    true
                }
            }
        ));

        // Add keyboard shortcut to cancel search
        let shortcut = gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(clone!(
                #[weak] imp,
                #[upgrade_or] glib::Propagation::Proceed,
                move |_, _| {
                    imp.search_entry.set_text("");
                    imp.view.grab_focus();

                    glib::Propagation::Stop
                }
            )))
        );

        let controller = gtk::ShortcutController::new();
        controller.add_shortcut(shortcut);

        imp.search_entry.add_controller(controller);

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Clear window
    //---------------------------------------
    pub fn remove_all(&self) {
        self.imp().model.remove_all();
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, parent: &impl IsA<gtk::Window>) {
        let imp = self.imp();

        self.set_transient_for(Some(parent));
        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
            // Read log lines
            let log_lines: Vec<LogLine> = Pacman::log().read().unwrap().as_ref()
                .map_or(vec![], |log| {
                    // Strip ANSI control sequences from log
                    static ANSI_EXPR: LazyLock<Regex> = LazyLock::new(|| {
                        Regex::new(r"\x1b(?:\[[0-9;]*m|\(B)").expect("Failed to compile Regex")
                    });

                    let log = ANSI_EXPR.replace_all(log, "");

                    // Parse log lines
                    static EXPR: LazyLock<Regex> = LazyLock::new(|| {
                        Regex::new(r"\[(.+?)T(.+?)\+.+?\] \[(.+?)\] (.+)").expect("Failed to compile Regex")
                    });

                    log.par_lines()
                        .filter_map(|line| {
                            EXPR.captures(line)
                                .map(|caps| LogLine {
                                    date: caps[1].to_string(),
                                    time: caps[2].to_string(),
                                    category: caps[3].to_string(),
                                    message: caps[4].to_string()
                                })
                        })
                        .collect()
                });

            // Populate column view
            imp.model.splice(0, 0, &log_lines.iter().rev()
                .map(LogObject::new)
                .collect::<Vec<LogObject>>()
            );
        }
    }
}

impl Default for LogWindow {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
