use std::sync::OnceLock;
use std::fs;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use regex::Regex;

use crate::log_object::LogObject;

//------------------------------------------------------------------------------
// MODULE: LogDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/log_dialog.ui")]
    pub struct LogDialog {
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::NoSelection>,
        #[template_child]
        pub(super) message_filter: TemplateChild<gtk::StringFilter>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for LogDialog {
        const NAME: &'static str = "LogDialog";
        type Type = super::LogDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            LogObject::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LogDialog {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
        }
    }

    impl WidgetImpl for LogDialog {}
    impl AdwDialogImpl for LogDialog {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: LogDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct LogDialog(ObjectSubclass<imp::LogDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LogDialog {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Files search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(@weak imp => move |entry| {
            imp.message_filter.set_search(Some(&entry.text()));
        }));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(@weak self as dialog, @weak imp => move |_| {
            let copy_text = imp.selection.iter::<glib::Object>().flatten()
                .map(|item| {
                    let log = item
                        .downcast::<LogObject>()
                        .expect("Could not downcast to 'LogObject'");

                    format!("[{date} {time}] {message}", date=log.date(), time=log.time(), message=log.message())
                })
                .collect::<Vec<String>>()
                .join("\n");

            dialog.clipboard().set_text(&copy_text);
        }));
    }

    //-----------------------------------
    // Populate widgets
    //-----------------------------------
    pub fn populate(&self, log_file: &str) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

        // Populate column view
        if let Ok(log) = fs::read_to_string(log_file) {
            static EXPR: OnceLock<Regex> = OnceLock::new();

            let expr = EXPR.get_or_init(|| {
                Regex::new("\\[(.+?)T(.+?)\\+.+?\\] (.+)").expect("Regex error")
            });

            let log_list: Vec<LogObject> = log.lines().rev()
                .filter_map(|s| {
                    expr.captures(s)
                        .filter(|caps| caps.len() == 4)
                        .map(|caps| {
                            LogObject::new(&caps[1], &caps[2], &caps[3])
                        })
                })
                .collect();

            imp.model.extend_from_slice(&log_list);
        }
    }
}

impl Default for LogDialog {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
