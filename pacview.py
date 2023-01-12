#!/usr/bin/env python

import gi, sys, os, datetime

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject

import pyalpm

from enum import IntFlag

#------------------------------------------------------------------------------
#-- FLAGS: PKGSTATUS
#------------------------------------------------------------------------------
class PkgStatus(IntFlag):
	EXPLICIT = 1
	DEPENDENCY = 2
	OPTIONAL = 4
	ORPHAN = 8
	INSTALLED = 15
	NONE = 16
	ALL = 31

#------------------------------------------------------------------------------
#-- CLASS: PKGOBJECT
#------------------------------------------------------------------------------
class PkgObject(GObject.Object):
	#-----------------------------------
	# Read/write properties
	#-----------------------------------
	pkg = GObject.Property(type=GObject.TYPE_PYOBJECT, default=None)
	status = GObject.Property(type=int, default=PkgStatus.NONE)
	date = GObject.Property(type=int, default=0)

	#-----------------------------------
	# Read-only properties
	#-----------------------------------
	@GObject.Property(type=str, default="")
	def name(self):
		return(self.pkg.name)

	@GObject.Property(type=str, default="")
	def version(self):
		return(self.pkg.version)

	@GObject.Property(type=str, default="")
	def repository(self):
		return(self.pkg.db.name)

	@GObject.Property(type=str, default="")
	def status_string(self):
		str_dict = { PkgStatus.EXPLICIT: "explicit", PkgStatus.DEPENDENCY: "dependency", PkgStatus.OPTIONAL: "optional", PkgStatus.ORPHAN: "orphan" }

		return(str_dict.get(self.status, ""))

	@GObject.Property(type=str, default="")
	def status_icon(self):
		icon_dict = { PkgStatus.EXPLICIT: "package-install", PkgStatus.DEPENDENCY: "package-installed-updated", PkgStatus.OPTIONAL: "package-installed-outdated", PkgStatus.ORPHAN: "package-purge" }

		return(icon_dict.get(self.status, ""))

	@GObject.Property(type=str, default="")
	def date_string(self):
		return(datetime.datetime.fromtimestamp(self.date).strftime("%Y/%m/%d %H:%M") if self.date != 0 else "")

	@GObject.Property(type=int, default=0)
	def size(self):
		return(self.pkg.isize)

	@GObject.Property(type=str, default="")
	def size_string(self):
		pkg_size = self.pkg.isize

		for unit in ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB']:
			if pkg_size < 1024.0 or unit == 'PiB':
				break
			pkg_size /= 1024.0
		
		return(f"{pkg_size:.1f} {unit}")

	@GObject.Property(type=str, default="")
	def description(self):
		return(self.pkg.desc)

	@GObject.Property(type=str, default="")
	def depends(self):
		return(self.pkg.depends)

	@GObject.Property(type=str, default="")
	def optdepends(self):
		return(self.pkg.optdepends)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pkg, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.pkg = pkg

#------------------------------------------------------------------------------
#-- CLASS: PKGCOLUMNVIEW
#------------------------------------------------------------------------------
@Gtk.Template(filename="/home/drakkar/Github/pacview/pkgcolumnview.ui")
class PkgColumnView(Gtk.Box):
	__gtype_name__ = "PkgColumnView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	filter_model = Gtk.Template.Child()
	model = Gtk.Template.Child()

	main_filter = Gtk.Template.Child()
	repo_filter = Gtk.Template.Child()
	status_filter = Gtk.Template.Child()
	search_filter = Gtk.Template.Child()

	name_factory = Gtk.Template.Child()
	version_factory = Gtk.Template.Child()
	repository_factory = Gtk.Template.Child()
	status_factory = Gtk.Template.Child()
	date_factory = Gtk.Template.Child()
	size_factory = Gtk.Template.Child()

	name_sorter = Gtk.Template.Child()
	version_sorter = Gtk.Template.Child()
	repository_sorter = Gtk.Template.Child()
	status_sorter = Gtk.Template.Child()
	date_sorter = Gtk.Template.Child()
	size_sorter = Gtk.Template.Child()

	status_bar = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	current_repo = GObject.Property(type=str, default="")
	current_status = GObject.Property(type=int, default=PkgStatus.ALL)
	current_search = GObject.Property(type=str, default="")

	search_by_name = GObject.Property(type=bool, default=True)
	search_by_desc = GObject.Property(type=bool, default=False)
	search_by_deps = GObject.Property(type=bool, default=False)
	search_by_optdeps = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind column sorters to sort functions
		self.name_sorter.set_sort_func(self.sort_by_str, "name")
		self.version_sorter.set_sort_func(self.sort_by_ver, "version")
		self.repository_sorter.set_sort_func(self.sort_by_str, "repository")
		self.status_sorter.set_sort_func(self.sort_by_str, "status_string")
		self.date_sorter.set_sort_func(self.sort_by_int, "date")
		self.size_sorter.set_sort_func(self.sort_by_int, "size")

		# Bind filters to filter functions
		self.repo_filter.set_filter_func(self.filter_by_repo)
		self.status_filter.set_filter_func(self.filter_by_status)
		self.search_filter.set_filter_func(self.filter_by_search)

		# Sort view by name (first) column
		self.view.sort_by_column(self.view.get_columns()[0], Gtk.SortType.ASCENDING)

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_item_setup_label(self, factory, item):
		item.set_child(Gtk.Label(halign=Gtk.Align.START))

	@Gtk.Template.Callback()
	def on_item_setup_iconlabel(self, factory, item):
		box = Gtk.Box()
		box.set_spacing(6)
		box.append(Gtk.Image())
		box.append(Gtk.Label(halign=Gtk.Align.START))
		item.set_child(box)

	@Gtk.Template.Callback()
	def on_item_bind_name(self, factory, item):
		item.get_child().get_first_child().set_from_icon_name("package-x-generic-symbolic")
		item.get_child().get_last_child().set_label(item.get_item().name)

	@Gtk.Template.Callback()
	def on_item_bind_version(self, factory, item):
		item.get_child().set_label(item.get_item().version)

	@Gtk.Template.Callback()
	def on_item_bind_repository(self, factory, item):
		item.get_child().set_label(item.get_item().repository)

	@Gtk.Template.Callback()
	def on_item_bind_status(self, factory, item):
		item.get_child().get_first_child().set_from_icon_name(item.get_item().status_icon)
		item.get_child().get_last_child().set_label(item.get_item().status_string)

	@Gtk.Template.Callback()
	def on_item_bind_date(self, factory, item):
		item.get_child().set_label(item.get_item().date_string)

	@Gtk.Template.Callback()
	def on_item_bind_size(self, factory, item):
		item.get_child().set_label(item.get_item().size_string)

	#-----------------------------------
	# Sorter functions
	#-----------------------------------
	def sort_by_str(self, item_a, item_b, prop):
		prop_a = item_a.get_property(prop)
		prop_b = item_b.get_property(prop)

		if prop_a < prop_b: return(-1)
		else:
			if prop_a > prop_b: return(1)
			else: return(0)

	def sort_by_ver(self, item_a, item_b, prop):
		return(pyalpm.vercmp(item_a.get_property(prop), item_b.get_property(prop)))

	def sort_by_int(self, item_a, item_b, prop):
		return(item_a.get_property(prop) - item_b.get_property(prop))

	#-----------------------------------
	# Filter signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_main_filter_changed(self, change, user_data):
		n_items = self.filter_model.get_n_items()
		self.status_bar.push(0, f'{n_items} matching package{"s" if n_items != 1 else ""}')

	#-----------------------------------
	# Filter functions
	#-----------------------------------
	def filter_by_repo(self, item):
		return(True if self.current_repo == "" else (item.repository == self.current_repo))

	def filter_by_status(self, item):
		return(item.status & self.current_status)

	def filter_by_search(self, item):
		if self.current_search == "":
			return(True)
		else:
			match_name = (self.current_search in item.name) if self.search_by_name else False
			match_desc = (self.current_search in item.description.lower()) if self.search_by_desc else False
			match_deps = ([s for s in item.depends if self.current_search in s] != []) if self.search_by_deps else False
			match_optdeps = ([s for s in item.optdepends if self.current_search in s] != []) if self.search_by_optdeps else False

			return(match_name or match_desc or match_deps or match_optdeps)

#------------------------------------------------------------------------------
#-- CLASS: SIDEBARLISTBOXROW
#------------------------------------------------------------------------------
@Gtk.Template(filename="/home/drakkar/Github/pacview/sidebarlistboxrow.ui")
class SidebarListBoxRow(Gtk.ListBoxRow):
	__gtype_name__ = "SidebarListBoxRow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	image = Gtk.Template.Child()
	label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	str_selection = GObject.Property(type=str, default="")
	int_selection = GObject.Property(type=int, default=PkgStatus.ALL)

	@GObject.Property(type=str)
	def icon_name(self):
		return(self.image.get_icon_name())

	@icon_name.setter
	def icon_name(self, value):
		self.image.set_from_icon_name(value)

	@GObject.Property(type=str)
	def label_text(self):
		return(self.label.get_text())

	@label_text.setter
	def label_text(self, value):
		self.label.set_text(value)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: MAINWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(filename="/home/drakkar/Github/pacview/mainwindow.ui")
class MainWindow(Adw.ApplicationWindow):
	__gtype_name__ = "MainWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	repo_listbox = Gtk.Template.Child()
	repo_listbox_all = Gtk.Template.Child()

	status_listbox = Gtk.Template.Child()
	status_listbox_installed = Gtk.Template.Child()

	search_bar = Gtk.Template.Child()
	search_entry = Gtk.Template.Child()

	pkg_columnview = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Add actions
		action_list = [
			( "search-toggle", self.on_search_toggle ),
			( "search-toggle-name", None, "", "true", self.on_search_params_toggle ),
			( "search-toggle-desc", None, "", "false", self.on_search_params_toggle ),
			( "search-toggle-deps", None, "", "false", self.on_search_params_toggle ),
			( "search-toggle-optdeps", None, "", "false", self.on_search_params_toggle ),
			( "refresh-dbs", self.on_refresh_dbs ),
			( "show-about", self.on_show_about ),
			( "quit-app", self.on_quit_app )
		]

		self.add_action_entries(action_list)

		# Add keyboard shortcuts
		app.set_accels_for_action("win.search-toggle", ["<ctrl>f"])
		app.set_accels_for_action("win.quit-app", ["<ctrl>q"])

		# Add rows to sidebar repository list box
		self.populate_sidebar_repos()

		# Select rows in sidebar list boxes
		self.repo_listbox.select_row(self.repo_listbox_all)
		self.status_listbox.select_row(self.status_listbox_installed)

		# Connect search bar to search entry
		self.search_bar.connect_entry(self.search_entry)

		# Add items to package column view
		self.pkg_columnview.model.splice(0, len(self.pkg_columnview.model), app.pkg_objects)

		self.pkg_columnview.main_filter.changed(Gtk.FilterChange.DIFFERENT)

		# Set initial focus on package column view
		self.set_focus(self.pkg_columnview.view)

	#-----------------------------------
	# Functions
	#-----------------------------------
	def populate_sidebar_repos(self):
		while(row := self.repo_listbox.get_row_at_index(1)):
			if row != self.repo_listbox_all: self.repo_listbox.remove(row)

		for db in app.db_names:
			self.repo_listbox.append(SidebarListBoxRow(icon_name="package-x-generic-symbolic", label_text=str.title(db), str_selection=db))

	#-----------------------------------
	# Action handlers
	#-----------------------------------
	def on_search_toggle(self, action, value, user_data):
		self.search_bar.set_search_mode(not self.search_bar.get_search_mode())

	def on_search_params_toggle(self, action, value, user_data):
		action.set_state(value)

		prop_name = str.replace(action.props.name, "search-toggle-", "search_by_")

		self.pkg_columnview.set_property(prop_name, value)

		self.pkg_columnview.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	def on_refresh_dbs(self, action, value, user_data):
		app.populate_pkg_objects()
		self.populate_sidebar_repos()

		self.pkg_columnview.model.splice(0, len(self.pkg_columnview.model), app.pkg_objects)

		self.pkg_columnview.main_filter.changed(Gtk.FilterChange.DIFFERENT)

	def on_show_about(self, action, value, user_data):
		about_window = Adw.AboutWindow(
			application_name="PacView",
			application_icon="software-properties",
			developer_name="draKKar1969",
			version="0.0.5",
			website="https://github.com/drakkar1969/pacview",
			developers=["draKKar1969"],
			designers=["draKKar1969"],
			license_type=Gtk.License.GPL_3_0,
			transient_for=self)

		about_window.show()

	def on_quit_app(self, action, value, user_data):
		self.close()

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_repo_selected(self, listbox, row):
		self.pkg_columnview.current_repo = row.str_selection

		self.pkg_columnview.repo_filter.changed(Gtk.FilterChange.DIFFERENT)

	@Gtk.Template.Callback()
	def on_status_selected(self, listbox, row):
		self.pkg_columnview.current_status = row.int_selection

		self.pkg_columnview.status_filter.changed(Gtk.FilterChange.DIFFERENT)

	@Gtk.Template.Callback()
	def on_search(self, entry):
		self.pkg_columnview.current_search = entry.get_text()

		self.pkg_columnview.search_filter.changed(Gtk.FilterChange.DIFFERENT)

#------------------------------------------------------------------------------
#-- CLASS: LAUNCHERAPP
#------------------------------------------------------------------------------
class LauncherApp(Adw.Application):
	#-----------------------------------
	# Variables
	#-----------------------------------
	db_names = []
	pkg_objects = []

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, **kwargs):
		super().__init__(**kwargs)

		# Connect signal handlers
		self.connect("activate", self.on_activate)

		self.populate_pkg_objects()

	def populate_pkg_objects(self):
		# Get path to pacman databases
		alpm_folder = "/var/lib/pacman"

		db_path = os.path.join(alpm_folder, "sync")

		# Build list of database names
		db_files = list(os.listdir(db_path)) if os.path.exists(db_path) else []
		self.db_names = [os.path.basename(db).split(".")[0] for db in db_files]

		# Get pyalpm handle
		alpm_handle = pyalpm.Handle("/", alpm_folder)

		self.pkg_objects.clear()

		# Build dictionary of names,install reasons of local packages
		local_db = alpm_handle.get_localdb()
		local_dict = dict([(pkg.name, pkg) for pkg in local_db.pkgcache])

		# Build list of PkgOBjects from packages in databases
		for db in self.db_names:
			sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

			if sync_db is not None:
				self.pkg_objects.extend([PkgObject(pkg) for pkg in sync_db.pkgcache])

		# Set status/date/required_by/optional_for for installed packages
		for i, obj in enumerate(self.pkg_objects):
			if obj.pkg.name in local_dict.keys():
				reason = local_dict[obj.pkg.name].reason

				if reason == 0: self.pkg_objects[i].status = PkgStatus.EXPLICIT
				else:
					if reason == 1:
						if local_dict[obj.pkg.name].compute_requiredby() != []:
							self.pkg_objects[i].status = PkgStatus.DEPENDENCY
						else:
							self.pkg_objects[i].status = PkgStatus.OPTIONAL if local_dict[obj.pkg.name].compute_optionalfor() != [] else PkgStatus.ORPHAN

				self.pkg_objects[i].date = local_dict[obj.pkg.name].installdate

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	def on_activate(self, app):
		self.main_window = MainWindow(application=app)
		self.main_window.present()

#------------------------------------------------------------------------------
#-- MAIN APP
#------------------------------------------------------------------------------
app = LauncherApp(application_id="com.github.PacView")
app.run(sys.argv)
