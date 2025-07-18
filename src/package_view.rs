use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::cmp::Ordering;
use std::borrow::Cow;
use std::marker::PhantomData;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, closure_local};

use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use raur::Raur;
use raur::ArcPackage;
use futures::future;

use crate::pkg_data::{PkgFlags, PkgData};
use crate::pkg_object::PkgObject;
use crate::search_bar::{SearchBar, SearchMode, SearchProp};
use crate::info_pane::InfoPane;
use crate::utils::tokio_runtime;
use crate::enum_traits::EnumExt;

//------------------------------------------------------------------------------
// GLOBAL VARIABLES
//------------------------------------------------------------------------------
thread_local! {
    pub static AUR_PKGS: RefCell<Vec<PkgObject>> = const { RefCell::new(vec![]) };
}

//------------------------------------------------------------------------------
// ENUM: PackageViewStatus
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PackageViewStatus")]
pub enum PackageViewStatus {
    #[default]
    Normal,
    PackageLoad,
    AURDownload,
}

//------------------------------------------------------------------------------
// ENUM: SortProp
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SortProp")]
pub enum SortProp {
    #[default]
    Name,
    Version,
    Repository,
    Status,
    InstallDate,
    InstalledSize,
    Groups,
}

impl EnumExt for SortProp {}

//------------------------------------------------------------------------------
// MODULE: PackageView
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PackageView)]
    #[template(resource = "/com/github/PacView/ui/package_view.ui")]
    pub struct PackageView {
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) loading_status: TemplateChild<adw::StatusPage>,

        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) pkg_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) aur_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) repo_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) factory: TemplateChild<gtk::BuilderListItemFactory>,
        #[template_child]
        pub(super) sorter: TemplateChild<gtk::CustomSorter>,

        #[template_child]
        pub(super) empty_status: TemplateChild<adw::StatusPage>,

        #[property(get)]
        #[template_child]
        pub(super) view: TemplateChild<gtk::ListView>,

        #[property(get, set, construct)]
        search_bar: RefCell<SearchBar>,
        #[property(get, set, construct)]
        info_pane: RefCell<InfoPane>,

        #[property(get, set)]
        n_items: Cell<u32>,
        #[property(get, set, builder(SortProp::default()))]
        sort_prop: Cell<SortProp>,
        #[property(get, set, default = true, construct)]
        sort_ascending: Cell<bool>,
        #[property(set = Self::set_status, builder(PackageViewStatus::default()))]
        status: PhantomData<PackageViewStatus>,

        #[property(get, set)]
        status_id: Cell<PkgFlags>,

        pub(super) aur_cache: Arc<TokioMutex<HashSet<ArcPackage>>>,
        pub(super) search_cancel_token: RefCell<Option<CancellationToken>>
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PackageView {
        const NAME: &'static str = "PackageView";
        type Type = super::PackageView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PackageView {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for PackageView {}
    impl BinImpl for PackageView {}

    impl PackageView{
        //---------------------------------------
        // Property setter
        //---------------------------------------
        pub fn set_status(&self, status: PackageViewStatus) {
            match status {
                PackageViewStatus::Normal => {
                    self.stack.set_visible_child_name("view");
                },
                PackageViewStatus::PackageLoad => {
                    self.loading_status.set_title("Loading Pacman Databases");
                    self.stack.set_visible_child_name("spinner");
                },
                PackageViewStatus::AURDownload => {
                    self.loading_status.set_title("Updating AUR Database");
                    self.stack.set_visible_child_name("spinner");
                }
            }
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PackageView
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PackageView(ObjectSubclass<imp::PackageView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PackageView {
    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set list view sorter function
        imp.sorter.set_sort_func(clone!(
            #[weak(rename_to = view)] self,
            #[upgrade_or] gtk::Ordering::Equal,
            move |item_a, item_b| {
                let pkg_a: &PkgObject = item_a
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                let pkg_b: &PkgObject = item_b
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                let sort = match view.sort_prop() {
                    SortProp::Name => { pkg_a.name().partial_cmp(&pkg_b.name()) },
                    SortProp::Version => { pkg_a.version().partial_cmp(&pkg_b.version()) },
                    SortProp::Repository => { pkg_a.repository().partial_cmp(&pkg_b.repository()) },
                    SortProp::Status => { pkg_a.status().partial_cmp(&pkg_b.status()) },
                    SortProp::InstallDate => { pkg_a.install_date().partial_cmp(&pkg_b.install_date()) },
                    SortProp::InstalledSize => { pkg_a.install_size().partial_cmp(&pkg_b.install_size()) },
                    SortProp::Groups => { pkg_a.groups().partial_cmp(&pkg_b.groups()) },
                }.unwrap_or(Ordering::Equal);

                if view.sort_ascending() {
                    sort
                } else {
                    sort.reverse()
                }.into()
            }
        ));

        // Set status filter function
        imp.status_filter.set_filter_func(clone!(
            #[weak(rename_to = view)] self,
            #[upgrade_or] false,
            move |item| {
                let pkg: &PkgObject = item
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                pkg.flags().intersects(view.status_id())
            }
        ));

        // Set search filter function
        imp.search_filter.set_filter_func(clone!(
            #[weak(rename_to = view)] self,
            #[upgrade_or] false,
            move |item| {
                let search_bar = view.search_bar();

                let term = search_bar.text().trim().to_lowercase();

                if term.is_empty() {
                    return true
                }
                
                let mode = search_bar.mode();
                let prop = search_bar.prop();

                let pkg: &PkgObject = item
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                let search_props: Cow<'_, [String]> = match prop {
                    SearchProp::Name => Cow::Owned(vec![pkg.name()]),
                    SearchProp::NameDesc => Cow::Owned(vec![pkg.name(), pkg.description().to_owned()]),
                    SearchProp::Groups => Cow::Owned(vec![pkg.groups()]),
                    SearchProp::Deps => Cow::Borrowed(pkg.depends()),
                    SearchProp::Optdeps => Cow::Borrowed(pkg.optdepends()),
                    SearchProp::Provides => Cow::Borrowed(pkg.provides()),
                    SearchProp::Files => Cow::Borrowed(pkg.files()),
                };

                match mode {
                    SearchMode::Exact => {
                        search_props.iter().any(|s| s.to_lowercase().eq(&term))
                    },
                    SearchMode::All => {
                        term.split_whitespace().all(|t| {
                            search_props.iter().any(|s| s.to_lowercase().contains(t))
                        })
                    },
                    SearchMode::Any => {
                        term.split_whitespace().any(|t| {
                            search_props.iter().any(|s| s.to_lowercase().contains(t))
                        })
                    },
                }
            }
        ));

        // Set search bar key capture widget
        self.search_bar().set_key_capture_widget(imp.view.upcast_ref());
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // List view selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak(rename_to = view)] self,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                view.set_n_items(n_items);
                view.imp().empty_status.set_visible(n_items == 0);
            }
        ));

        // List view selected item property notify signal
        imp.selection.connect_selected_item_notify(clone!(
            #[weak(rename_to = view)] self,
            move |selection| {
                let pkg = selection.selected_item()
                    .and_downcast::<PkgObject>();

                view.info_pane().set_pkg(pkg);
            }
        ));

        // List view activate signal
        imp.view.connect_activate(clone!(
            #[weak(rename_to = view)] self,
            move |_, index| {
                let pkg = view.imp().selection.item(index)
                    .and_downcast::<PkgObject>();

                let info_pane = view.info_pane();

                if pkg != info_pane.pkg() {
                    info_pane.set_pkg(pkg);
                }
            }
        ));

        // Sort prop property notify signal
        self.connect_sort_prop_notify(|view| {
            view.imp().sorter.changed(gtk::SorterChange::Different);
        });

        // Sort ascending property notify signal
        self.connect_sort_ascending_notify(|view| {
            view.imp().sorter.changed(gtk::SorterChange::Inverted);
        });

        // Search bar changed signal
        self.search_bar().connect_closure("changed", false, closure_local!(
            #[weak(rename_to = view)] self,
            move |_: SearchBar| {
                view.imp().search_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Search bar AUR Search signal
        self.search_bar().connect_closure("aur-search", false, closure_local!(
            #[weak(rename_to = view)] self,
            move |search_bar: &SearchBar| {
                view.search_in_aur(search_bar);
            }
        ));

        // Search bar enabled property notify signal
        self.search_bar().connect_enabled_notify(clone!(
            #[weak(rename_to = view)] self,
            move |bar| {
                if !bar.enabled() {
                    view.cancel_aur_search();

                    view.imp().view.grab_focus();
                }
            }
        ));
    }

    //---------------------------------------
    // Public sidebar filter functions
    //---------------------------------------
    pub fn repo_filter_changed(&self, repo_id: Option<&str>) {
        self.imp().repo_filter.set_search(repo_id);
    }

    pub fn status_filter_changed(&self, status_id: PkgFlags) {
        self.set_status_id(status_id);

        self.imp().status_filter.changed(gtk::FilterChange::Different);
    }

    //---------------------------------------
    // Do search async helper function
    //---------------------------------------
    async fn do_search_async(
        term: &str,
        prop: SearchProp,
        aur_cache: &Arc<TokioMutex<HashSet<raur::ArcPackage>>>
    ) -> Result<Vec<raur::ArcPackage>, raur::Error> {
        let handle = raur::Handle::new();

        // Return if query arg too small
        if term.len() < 2 {
            return Err(raur::Error::Aur(String::from("Query arg too small.")))
        }

        // Return if attempting to search by files
        if prop == SearchProp::Files {
            return Err(raur::Error::Aur(String::from("Cannot search by files.")))
        }

        // Set search mode
        let search_by = match prop {
            SearchProp::Name => raur::SearchBy::Name,
            SearchProp::NameDesc => raur::SearchBy::NameDesc,
            SearchProp::Groups => raur::SearchBy::Groups,
            SearchProp::Deps => raur::SearchBy::Depends,
            SearchProp::Optdeps => raur::SearchBy::OptDepends,
            SearchProp::Provides => raur::SearchBy::Provides,
            SearchProp::Files => unreachable!(),
        };

        // Search for AUR packages
        let search_results = future::join_all(term.split_whitespace()
            .map(|t| handle.search_by(t, search_by))
        )
        .await;

        let mut search_names: HashSet<String> = HashSet::new();

        for result in search_results {
            search_names.extend(result?.into_iter().map(|pkg| pkg.name));
        }

        // Get AUR package info using cache
        let aur_pkg_list = handle.cache_info(
            &mut *aur_cache.lock().await,
            &search_names.iter().collect::<Vec<&String>>()
        )
        .await?;

        Ok(aur_pkg_list)
    }

    //---------------------------------------
    // Public reset AUR search function
    //---------------------------------------
    pub fn reset_aur_search(&self) {
        // Cancel ongoing AUR search if any
        self.cancel_aur_search();

        // Clear AUR search results
        self.imp().aur_model.remove_all();

        AUR_PKGS.replace(vec![]);
    }

    //---------------------------------------
    // Public cancel AUR search function
    //---------------------------------------
    pub fn cancel_aur_search(&self) {
        let imp = self.imp();

        if let Some(token) = imp.search_cancel_token.take() {
            token.cancel();
        }
    }

    //---------------------------------------
    // Public search in AUR function
    //---------------------------------------
    pub fn search_in_aur(&self, search_bar: &SearchBar) {
        let imp = self.imp();

        let term = search_bar.text().trim().to_lowercase();
        let prop = search_bar.prop();

        // Reset AUR search
        self.reset_aur_search();

        // Return if search term is empty
        if term.is_empty() {
            return
        }

        // Show search spinner
        search_bar.set_searching(true);

        // Get AUR cache (clone Arc)
        let aur_cache = Arc::clone(&imp.aur_cache);

        // Create and store search cancel token
        let cancel_token = CancellationToken::new();

        let cancel_token_clone = cancel_token.clone();

        imp.search_cancel_token.replace(Some(cancel_token));

        // Create tokio task to search AUR
        let search_future = tokio_runtime::runtime().spawn(
            async move {
                tokio::select! {
                    () = cancel_token_clone.cancelled() => { Ok(vec![]) },
                    res = Self::do_search_async(&term, prop, &aur_cache) => {
                        res.map(|aur_list| {
                            aur_list.iter().map(|pkg| PkgData::from_aur(pkg)).collect()
                        })
                    }
                }
            }
        );

        // Await task
        glib::spawn_future_local(clone!(
            #[weak] imp,
            #[weak] search_bar,
            async move {
                // Spawn tokio task to search AUR
                let result = search_future.await
                    .expect("Failed to complete tokio task");

                match result {
                    // Get AUR search results
                    Ok(data_list) => {
                        if search_bar.enabled() {
                            let pkg_list: Vec<PkgObject> = data_list.into_iter()
                                .map(|data| PkgObject::new(data, None))
                                .collect();

                            imp.aur_model.splice(0, imp.aur_model.n_items(), &pkg_list);

                            AUR_PKGS.replace(pkg_list);
                        }

                        search_bar.set_aur_status(Ok(()));
                    },
                    Err(error) => {
                        search_bar.set_aur_status(Err(error.to_string()));
                    }
                }

                // Remove stored search cancel token
                imp.search_cancel_token.replace(None);

                // Hide search spinner
                search_bar.set_searching(false);
            }
        ));
    }

    //---------------------------------------
    // Public package functions
    //---------------------------------------
    pub fn splice_packages(&self, pkg_slice: &[PkgObject]) {
        let imp = self.imp();

        imp.pkg_model.splice(0, imp.pkg_model.n_items(), pkg_slice);
    }

    pub fn show_updates(&self, update_map: &HashMap<String, String>) {
        for pkg in self.imp().pkg_model.iter::<PkgObject>().flatten() {
            if let Some(new_version) = update_map.get(&pkg.name()) {
                pkg.set_update_version(Some(new_version.to_owned()));
            }
        }
    }

    //---------------------------------------
    // Public copy list function
    //---------------------------------------
    pub fn copy_list(&self) -> String {
        let body = self.imp().selection.iter::<glib::Object>()
            .flatten()
            .map(|item| {
                let pkg = item
                    .downcast::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                format!("|{name}|{version}|{repo}|{status}|{size}|{groups}|",
                    name=pkg.name(),
                    version=pkg.version(),
                    repo=pkg.repository(),
                    status=pkg.status(),
                    size=pkg.install_size_string(),
                    groups=pkg.groups()
                )
            })
            .collect::<Vec<String>>()
            .join("\n");

        format!("## Package List\n|Package Name|Version|Repository|Status|Installed Size|Groups|\n|---|---|---|---|---:|---|\n{body}")
    }
}

impl Default for PackageView {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
