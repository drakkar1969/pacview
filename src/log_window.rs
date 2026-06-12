use std::{cell::Cell, os::unix::fs::MetadataExt};
use std::sync::LazyLock;
use std::fs;
use std::fmt::Write as _;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, Propagation};
use gdk::{Key, ModifierType};

use regex::Regex;
use size::Size;

use crate::{
    utils::Pacman,
    log_object::{LogLine, LogObject}
};

//------------------------------------------------------------------------------
// MODULE: LogWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LogWindow)]
    #[template(resource = "/com/github/PacView/ui/log_window.ui")]
    pub struct LogWindow {
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,

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
        #[template_child]
        pub(super) size_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        loading: Cell<bool>,
        #[property(get, set)]
        packages_only: Cell<bool>,
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

            // Install actions
            Self::install_actions(klass);

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
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
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Packages only property action
            klass.install_property_action("log.packages-only", "packages-only");

            // Copy action
            klass.install_action("log.copy", None, |window, _, _| {
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
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");

            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |window| {
                window.imp().search_bar.set_search_mode(true);

                Propagation::Stop
            });

            // Filter package events key binding
            klass.add_binding_action(Key::P, ModifierType::CONTROL_MASK, "log.packages-only");

            // Copy key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "log.copy");
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

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.search_filter.set_search(Some(&entry.text()));
            }
        ));

        // Packages only property notify signal
        self.connect_packages_only_notify(|window| {
            window.imp().package_filter.changed(gtk::FilterChange::Different);
        });

        // Loading property notify signal
        self.connect_loading_notify(|window| {
            let imp = window.imp();

            imp.stack.set_visible_child_name(
                if imp.selection.n_items() == 0 {
                    if window.loading() { "loading" } else { "empty" }
                } else {
                    "view"
                }
            );
        });

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak(rename_to = window)] self,
            move |selection, _, _, _| {
                let imp = window.imp();

                let n_items = selection.n_items();

                imp.stack.set_visible_child_name(
                    if n_items == 0 {
                        if window.loading() { "loading" } else { "empty" }
                    } else {
                        "view"
                    }
                );

                imp.footer_label.set_label(&format!("{n_items} line{}", if n_items == 1 { "" } else { "s" }));

                window.action_set_enabled("log.copy", n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search bar key capture widget
        imp.search_bar.set_key_capture_widget(Some(&imp.view.get()));

        // Bind search button state to search bar visibility
        imp.search_button.bind_property("active", &imp.search_bar.get(), "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();

        // Set package filter function
        imp.package_filter.set_filter_func(clone!(
            #[weak(rename_to = window)] self,
            #[upgrade_or] false,
            move |item| {
                if window.packages_only() {
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

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Populate window
    //---------------------------------------
    pub fn populate(&self) {
        let imp = self.imp();

        self.set_loading(true);

        // Clear view
        imp.model.remove_all();

        // Spawn task to read log
        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || {
            if let Some(log) = Pacman::log().read().unwrap().as_ref() {
                // Parse log lines
                static EXPR: LazyLock<Regex> = LazyLock::new(|| {
                    Regex::new(r"\[(.+?)T(.+?)\+.+?\] \[(.+?)\] (.+)")
                        .expect("Failed to compile Regex")
                });

                let log_lines: Vec<_> = log
                    .split('\n')
                    .collect();

                let mut first = true;

                for chunk in log_lines.chunks(1000) {
                    let lines: Vec<LogLine> = chunk.into_iter()
                        .filter_map(|line| {
                            EXPR.captures(line)
                                .map(|caps| LogLine {
                                    date: caps[1].to_string(),
                                    time: caps[2].to_string(),
                                    category: caps[3].to_string(),
                                    message: caps[4].trim().to_string()
                                })
                        })
                        .collect();

                    sender.send_blocking((lines, first))
                        .expect("Failed to send through channel");

                    if first { first = false };
                }
            }
        });

        // Attach package load task receiver
        glib::spawn_future_local(clone!(
            #[weak(rename_to = window)] self,
            async move {
                let imp = window.imp();

                // Populate column view
                while let Ok((log_lines, first)) = receiver.recv().await {
                    imp.model.splice(0, 0, &log_lines.iter().rev()
                        .map(LogObject::new)
                        .collect::<Vec<LogObject>>()
                    );

                    if first {
                        window.set_loading(false);
                    }
                }

                // Get log file size
                let size = 512u64 * fs::metadata(&Pacman::config().log_file)
                    .map(|metadata| metadata.blocks())
                    .unwrap_or_default();

                imp.size_label.set_label(&format!("Log file size: {}", Size::from_bytes(size)));
            }
        ));
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
