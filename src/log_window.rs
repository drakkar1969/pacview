use std::cell::RefCell;
use std::sync::OnceLock;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use regex::Regex;
use rayon::{str::ParallelString, iter::ParallelIterator};

use crate::log_object::LogObject;

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
        pub(super) header_sub_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) package_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

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
        pub(super) error_label: TemplateChild<gtk::Label>,

        pub(super) bindings: RefCell<Vec<glib::Binding>>,
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

            // Add find key binding
            klass.add_binding(gdk::Key::F, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if !imp.search_entry.has_focus() {
                    imp.search_entry.grab_focus();
                }

                glib::Propagation::Stop
            });

            // Add filter package events key binding
            klass.add_binding(gdk::Key::P, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                imp.package_button.set_active(!imp.package_button.is_active());

                glib::Propagation::Stop
            });

            // Add copy key binding
            klass.add_binding(gdk::Key::C, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.copy_button.is_sensitive() {
                    imp.copy_button.emit_clicked();
                }

                glib::Propagation::Stop
            });
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

            obj.setup_widgets();
            obj.setup_controllers();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for LogWindow {}
    impl WindowImpl for LogWindow {}
    impl AdwWindowImpl for LogWindow {}
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
    // New function
    //---------------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);

        // Add close window shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::NamedAction::new("window.close"))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
    }

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
            move |package_button| {
                if package_button.is_active() {
                    imp.package_filter.set_filter_func(move |item| {
                        let msg = item
                            .downcast_ref::<LogObject>()
                            .expect("Could not downcast to 'LogObject'")
                            .message();

                        msg.starts_with("installed ") || msg.starts_with("removed ") || msg.starts_with("upgraded ") || msg.starts_with("downgraded ")
                    });
                } else {
                    imp.package_filter.unset_filter_func();
                }
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            move |_| {
                let body = imp.selection.iter::<glib::Object>().flatten()
                    .map(|item| {
                        let log = item
                            .downcast::<LogObject>()
                            .expect("Could not downcast to 'LogObject'");

                        format!("|{date}|{time}|{category}|{message}|",
                            date=log.date(),
                            time=log.time(),
                            category=log.category(),
                            message=log.message())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                window.clipboard().set_text(&
                    format!("## Log Messages\n|Date|Time|Category|Message|\n|---|---|---|---|\n{body}")
                );
            }
        ));
    }

    //---------------------------------------
    // Clear window
    //---------------------------------------
    pub fn clear(&self) {
        for binding in self.imp().bindings.take() {
            binding.unbind();
        }

        self.imp().model.remove_all();
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, log: Option<&str>) {
        let imp = self.imp();

        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
            // Define local struct
            struct LogLine {
                date: String,
                time: String,
                category: String,
                message: String
            }

            // Read log lines
            let log_lines = log.map_or(vec![], |log| {
                // Strip ANSI control sequences from log
                static ANSI_EXPR: OnceLock<Regex> = OnceLock::new();

                let ansi_expr = ANSI_EXPR.get_or_init(|| {
                    Regex::new(r"\x1b(?:\[[0-9;]*m|\(B)")
                        .expect("Regex error")
                });

                let log = ansi_expr.replace_all(log, "");

                // Parse log lines
                static EXPR: OnceLock<Regex> = OnceLock::new();

                let expr = EXPR.get_or_init(|| {
                    Regex::new(r"\[(.+?)T(.+?)\+.+?\] \[(.+?)\] (.+)")
                        .expect("Regex error")
                });

                log.par_lines()
                    .filter_map(|line|
                        expr.captures(line)
                            .map(|caps| LogLine {
                                date: caps[1].to_string(),
                                time: caps[2].to_string(),
                                category: caps[3].to_string(),
                                message: caps[4].to_string()
                            })
                    )
                    .collect()
            });

            // Populate column view
            imp.model.splice(0, 0, &log_lines.iter().rev()
                .map(|line| LogObject::new(&line.date, &line.time, &line.category, &line.message))
                .collect::<Vec<LogObject>>()
            );

            // Bind view count to header sub label
            let label_binding = imp.selection.bind_property("n-items", &imp.header_sub_label.get(), "label")
            .transform_to(|_, n_items: u32|
                Some(format!("{n_items} line{}", if n_items != 1 {"s"} else {""}))
            )
            .sync_create()
            .build();

            // Bind view count to copy button state
            let copy_binding = imp.selection.bind_property("n-items", &imp.copy_button.get(), "sensitive")
                .transform_to(|_, n_items: u32|Some(n_items > 0))
                .sync_create()
                .build();

            imp.bindings.replace(vec![label_binding, copy_binding]);
        }
    }
}
