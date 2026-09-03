use std::cell::{Cell, RefCell};
use std::sync::LazyLock;
use std::fs;
use std::io;
use std::io::{BufRead, BufReader};
use std::fmt::Write as _;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::{clone, Propagation};
use gdk::{Key, ModifierType};

use itertools::Itertools;
use strum::{EnumIter, IntoEnumIterator, AsRefStr};
use regex::Regex;
use size::Size;

use crate::{
    utils::{Pacman, AppInfoExt},
    log_object::{LogLine, LogObject}
};

//------------------------------------------------------------------------------
// ENUM: LogSearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, EnumIter, AsRefStr)]
#[repr(u32)]
#[enum_type(name = "LogSearchMode")]
pub enum LogSearchMode {
    #[default]
    Messages,
    Packages,
    Exact,
}

//------------------------------------------------------------------------------
// MODULE: LogDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LogDialog)]
    #[template(resource = "/com/github/PacView/ui/log_dialog.ui")]
    pub struct LogDialog {
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) search_mode_label: TemplateChild<gtk::Label>,

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
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,

        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) size_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        is_loaded: Cell<bool>,
        #[property(get, set, builder(LogSearchMode::default()))]
        search_mode: Cell<LogSearchMode>,

        pub(super) search_term: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for LogDialog {
        const NAME: &'static str = "LogDialog";
        type Type = super::LogDialog;
        type ParentType = adw::Dialog;

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
    impl ObjectImpl for LogDialog {
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

    impl WidgetImpl for LogDialog {}
    impl AdwDialogImpl for LogDialog {}

    impl LogDialog {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Search mode property action
            klass.install_property_action("search.set-mode", "search-mode");

            // Cycle search mode action
            klass.install_action("search.cycle-mode", None, |dialog, _, _| {
                let new_mode = LogSearchMode::iter().cycle()
                    .skip_while(|&mode| mode != dialog.search_mode())
                    .nth(1)
                    .expect("Failed to get 'LogSearchMode'");

                dialog.set_search_mode(new_mode);
            });

            // Reverse cycle search mode action
            klass.install_action("search.reverse-cycle-mode", None, |dialog, _, _| {
                let new_mode = LogSearchMode::iter().rev().cycle()
                    .skip_while(|&mode| mode != dialog.search_mode())
                    .nth(1)
                    .expect("Failed to get 'LogSearchMode'");

                dialog.set_search_mode(new_mode);
            });

            // Open action
            klass.install_action_async("log.open", None, async |_, _, _| {
                let log_file = &Pacman::config().read().unwrap().log_file;

                AppInfoExt::open_with_default_app(log_file).await;
            });

            // Copy action
            klass.install_action("log.copy", None, |dialog, _, _| {
                let mut output = String::from("## Log Messages\n|Date|Time|Category|Message|\n|---|---|---|---|\n");

                for log in dialog.imp().selection.iter::<glib::Object>()
                    .filter_map(|item| item.ok().and_downcast::<LogObject>()) {
                        writeln!(output, "|{date}|{time}|{message}|",
                            date=log.date(),
                            time=log.time(),
                            message=log.message()
                        )
                        .unwrap();
                    }

                dialog.clipboard().set_text(&output);
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |dialog| {
                dialog.imp().search_bar.set_search_mode(true);

                Propagation::Stop
            });

            // Cycle search mode key bindings
            klass.add_binding_action(Key::M, ModifierType::CONTROL_MASK, "search.cycle-mode");
            klass.add_binding_action(Key::M, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "search.reverse-cycle-mode");

            // Open key binding
            klass.add_binding_action(Key::O, ModifierType::CONTROL_MASK, "log.open");

            // Copy key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "log.copy");
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: LogDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct LogDialog(ObjectSubclass<imp::LogDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl LogDialog {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search bar search mode enabled signal
        imp.search_bar.connect_search_mode_enabled_notify(clone!(
            #[weak] imp,
            move |bar| {
                if !bar.is_search_mode() {
                    imp.view.grab_focus();
                }
            }
        ));

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.search_term.replace(entry.text().trim().to_lowercase());

                imp.search_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Search mode property notify signal
        self.connect_search_mode_notify(|dialog| {
            let imp = dialog.imp();

            imp.search_mode_label.set_label(dialog.search_mode().as_ref());

            imp.search_filter.changed(gtk::FilterChange::Different);
        });

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak(rename_to = dialog)] self,
            move |selection, _, _, _| {
                let imp = dialog.imp();

                let n_items = selection.n_items();

                imp.stack.set_visible_child_name(
                    if n_items == 0 { "empty" } else { "view" }
                );

                imp.count_label.set_label(&format!("{n_items} line{}", if n_items == 1 { "" } else { "s" }));

                dialog.action_set_enabled("log.copy", n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search bar key capture widget and connect entry
        imp.search_bar.set_key_capture_widget(Some(&imp.view.get()));
        imp.search_bar.connect_entry(&imp.search_entry.get());

        // Bind search button state to search bar visibility
        imp.search_button.bind_property("active", &imp.search_bar.get(), "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();

        // Set search filter function
        imp.search_filter.set_filter_func(clone!(
            #[weak(rename_to = dialog)] self,
            #[upgrade_or] false,
            move |item| {
                let search_term = dialog.imp().search_term.borrow();

                if search_term.is_empty() {
                    return true;
                }

                let msg = item
                    .downcast_ref::<LogObject>()
                    .expect("Failed to downcast to 'LogObject'")
                    .message();

                let is_match = |prop: &str| -> bool {
                    prop.as_bytes()
                        .windows(search_term.len())
                        .any(|window| window.eq_ignore_ascii_case(search_term.as_bytes()))
                };

                if dialog.search_mode() == LogSearchMode::Messages {
                    is_match(&msg)
                } else {
                    let Some((prefix, package, _)) = msg.splitn(3, ' ').collect_tuple() else {
                        return false;
                    };

                    if !(prefix == "installed" || prefix == "reinstalled" || prefix == "removed" || prefix == "upgraded" || prefix == "downgraded") {
                        return false;
                    }

                    if dialog.search_mode() == LogSearchMode::Packages {
                        is_match(package)
                    } else {
                        package.eq_ignore_ascii_case(&search_term)
                    }
                }
            }
        ));

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Populate dialog
    //---------------------------------------
    fn populate(&self) {
        let imp = self.imp();

        // Clear view
        imp.model.remove_all();

        // Spawn task to read log
        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || {
            // Read log file (strip ANSI codes)
            static ANSI_EXPR: LazyLock<Regex> = LazyLock::new(|| {
                Regex::new(r"\x1b(?:\[[0-9;]*m|\(B)").expect("Failed to compile Regex")
            });

            let log_file = fs::File::open(&Pacman::config().read().unwrap().log_file)?;

            let reader = BufReader::new(log_file);

            let mut log_lines: Vec<String> = reader.lines()
                .map_while(Result::ok)
                .map(|mut line| {
                    // Strip ANSI codes
                    if line.contains('\x1b') {
                        line = ANSI_EXPR.replace_all(&line, "").into_owned();
                    }

                    line
                })
                .collect();

            log_lines.reverse();

            // Parse log lines
            for chunk in log_lines.chunks(1000) {
                sender.send_blocking(
                    chunk.iter()
                        .filter_map(|line| LogLine::parse(line))
                        .collect::<Vec<LogLine>>()
                )
                .expect("Failed to send through channel");
            }

            Ok::<(), io::Error>(())
        });

        // Attach log task receiver
        glib::spawn_future_local(clone!(
            #[weak(rename_to = dialog)] self,
            async move {
                let imp = dialog.imp();

                // Populate column view
                while let Ok(log_lines) = receiver.recv().await {
                    imp.model.splice(imp.model.n_items(), 0, &log_lines.iter()
                        .map(LogObject::new)
                        .collect::<Vec<LogObject>>()
                    );
                }

                // Get log file size
                let size = fs::metadata(&Pacman::config().read().unwrap().log_file)
                    .map(|metadata| metadata.len())
                    .unwrap_or_default();

                imp.size_label.set_label(&format!("Log file size: {}", Size::from_bytes(size)));
            }
        ));
    }

    //---------------------------------------
    // Show dialog
    //---------------------------------------
    pub fn show(&self, parent: Option<&impl IsA<gtk::Widget>>) {
        self.present(parent);

        glib::idle_add_local_once(clone!(
            #[weak(rename_to = dialog)] self,
            move || {
                if !dialog.is_loaded() {
                    dialog.populate();

                    dialog.set_is_loaded(true);
                }
            }
        ));
    }
}

impl Default for LogDialog {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
