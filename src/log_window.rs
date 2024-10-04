use std::sync::OnceLock;
use std::fs;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use regex::Regex;

use crate::log_object::LogObject;

//------------------------------------------------------------------------------
// MODULE: LogWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
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
        pub(super) overlay_label: TemplateChild<gtk::Label>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for LogWindow {
        const NAME: &'static str = "LogWindow";
        type Type = super::LogWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            LogObject::ensure_type();

            klass.bind_template();

            klass.add_shortcut(&gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string("Escape"),
                Some(gtk::CallbackAction::new(|widget, _| {
                    let window = widget
                        .downcast_ref::<crate::log_window::LogWindow>()
                        .expect("Could not downcast to 'BackupWindow'");

                    window.close();

                    glib::Propagation::Proceed
                }))
            ))
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LogWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
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
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

        // Bind view count to header sub label
        imp.filter_model.bind_property("n-items", &imp.header_sub_label.get(), "label")
            .transform_to(|_, n_items: u32| {
                Some(format!("{n_items} line{}", if n_items != 1 {"s"} else {""}))
            })
            .sync_create()
            .build();

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
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
                static EXPR: OnceLock<Regex> = OnceLock::new();

                let expr = EXPR.get_or_init(|| {
                    Regex::new(r"\[ALPM\] installed|removed|upgraded|downgraded .+")
                        .expect("Regex error")
                });

                if package_button.is_active() {
                    imp.package_filter.set_filter_func(move |item| {
                        let msg = item
                            .downcast_ref::<LogObject>()
                            .expect("Could not downcast to 'LogObject'");

                        expr.is_match(&msg.message())
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
                let copy_text = imp.selection.iter::<glib::Object>().flatten()
                    .map(|item| {
                        let log = item
                            .downcast::<LogObject>()
                            .expect("Could not downcast to 'LogObject'");

                        format!("[{date} {time}] {message}", date=log.date(), time=log.time(), message=log.message())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                window.clipboard().set_text(&copy_text);
            }
        ));
    }

    //-----------------------------------
    // Show window
    //-----------------------------------
    pub fn show(&self, log_file: &str) {
        let imp = self.imp();

        self.present();

        let log_file = log_file.to_string();

        // Spawn thread to populate column view
        glib::spawn_future_local(clone!(
            #[weak] imp,
            async move {
                if let Ok(log) = fs::read_to_string(log_file) {
                    // Strip ANSI control sequences from log
                    static ANSI_EXPR: OnceLock<Regex> = OnceLock::new();

                    let ansi_expr = ANSI_EXPR.get_or_init(|| {
                        Regex::new(r"\x1b(?:\[[0-9;]*m|\(B)")
                            .expect("Regex error")
                    });

                    let log = ansi_expr.replace_all(&log, "");

                    // Populate column view
                    static EXPR: OnceLock<Regex> = OnceLock::new();

                    let expr = EXPR.get_or_init(|| {
                        Regex::new(r"\[(.+?)T(.+?)\+.+?\] (.+)")
                            .expect("Regex error")
                    });

                    let log_lines: Vec<LogObject> = log.lines().rev()
                        .filter_map(|s| {
                            expr.captures(s)
                                .map(|caps| LogObject::new(&caps[1], &caps[2], &caps[3]))
                        })
                        .collect();

                    imp.model.extend_from_slice(&log_lines);
                } else {
                    // Show overlay error label
                    imp.overlay_label.set_visible(true);
                }
            }
        ));
    }
}
