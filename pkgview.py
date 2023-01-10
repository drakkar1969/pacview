#!/usr/bin/env python

import gi, sys, os, datetime

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject

import pyalpm

from enum import IntEnum

#------------------------------------------------------------------------------
#-- ENUM: PKGSTATUS
#------------------------------------------------------------------------------
class PkgStatus(IntEnum):
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
	# Properties
	#-----------------------------------
	pkg = GObject.Property(type=GObject.TYPE_PYOBJECT, default=None)

	@GObject.Property(type=str, default="")
	def name(self):
		return(self.pkg.name)

	@GObject.Property(type=str, default="")
	def version(self):
		return(self.pkg.version)

	@GObject.Property(type=str, default="")
	def repository(self):
		return(self.pkg.db.name)

	status = GObject.Property(type=int, default=PkgStatus.NONE)

	@GObject.Property(type=str, default="")
	def status_string(self):
		str_dict = { PkgStatus.EXPLICIT: "explicit", PkgStatus.DEPENDENCY: "dependency", PkgStatus.OPTIONAL: "optional", PkgStatus.ORPHAN: "orphan" }

		return(str_dict.get(self.status, ""))

	@GObject.Property(type=str, default="")
	def status_icon(self):
		icon_dict = { PkgStatus.EXPLICIT: "package-install", PkgStatus.DEPENDENCY: "package-installed-updated", PkgStatus.OPTIONAL: "package-installed-outdated", PkgStatus.ORPHAN: "package-purge" }

		return(icon_dict.get(self.status, ""))

	date = GObject.Property(type=int, default=0)

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

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pkg, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.pkg = pkg

#------------------------------------------------------------------------------
#-- CLASS: PKGCOLUMNVIEW
#------------------------------------------------------------------------------
@Gtk.Template(filename="/home/drakkar/Github/pkgview/pkgcolumnview.ui")
class PkgColumnView(Gtk.ScrolledWindow):
	__gtype_name__ = "PkgColumnView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	model = Gtk.Template.Child()
	pkg_filter = Gtk.Template.Child()

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

	#-----------------------------------
	# Properties
	#-----------------------------------
	repo_filter = GObject.Property(type=str, default="")
	status_filter = GObject.Property(type=int, default=PkgStatus.ALL)
	search_filter = GObject.Property(type=str, default="")

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

		# Bind filter to filter function
		self.pkg_filter.set_filter_func(self.filter_pkgs)

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
	# Filter function
	#-----------------------------------
	def filter_pkgs(self, item):
		match_repo = True if self.repo_filter == "" else (item.repository == self.repo_filter)
		match_status = (item.status & self.status_filter)
		match_search = True if self.search_filter == "" else (self.search_filter in item.name)

		return(match_repo and (match_status and match_search))

#------------------------------------------------------------------------------
#-- CLASS: FILTERLISTBOXROW
#------------------------------------------------------------------------------
@Gtk.Template(filename="/home/drakkar/Github/pkgview/filterlistboxrow.ui")
class FilterListBoxRow(Gtk.ListBoxRow):
	__gtype_name__ = "FilterListBoxRow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	image = Gtk.Template.Child()
	label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	str_filter = GObject.Property(type=str, default="")
	int_filter = GObject.Property(type=int, default=PkgStatus.ALL)

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
@Gtk.Template(filename="/home/drakkar/Github/pkgview/mainwindow.ui")
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

		for db in app.db_names:
			self.repo_listbox.append(FilterListBoxRow(icon_name="package-x-generic-symbolic", label_text=str.title(db), str_filter=db))

		self.repo_listbox.connect("row-selected", self.on_repo_changed)
		self.status_listbox.connect("row-selected", self.on_status_changed)

		self.repo_listbox.select_row(self.repo_listbox_all)
		self.status_listbox.select_row(self.status_listbox_installed)

		self.pkg_columnview.model.splice(0, 0, app.pkg_objects)

	def on_repo_changed(self, listbox, row):
		self.pkg_columnview.repo_filter = row.str_filter

		self.pkg_columnview.pkg_filter.changed(Gtk.FilterChange.DIFFERENT)

	def on_status_changed(self, listbox, row):
		self.pkg_columnview.status_filter = row.int_filter

		self.pkg_columnview.pkg_filter.changed(Gtk.FilterChange.DIFFERENT)

	@Gtk.Template.Callback()
	def on_search(self, entry):
		self.pkg_columnview.search_filter = entry.get_text()

		self.pkg_columnview.pkg_filter.changed(Gtk.FilterChange.DIFFERENT)

#------------------------------------------------------------------------------
#-- CLASS: LAUNCHERAPP
#------------------------------------------------------------------------------
class LauncherApp(Adw.Application):

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, **kwargs):
		super().__init__(**kwargs)

		# Connect signal handlers
		self.connect("activate", self.on_activate)

		# Get path to pacman databases
		alpm_folder = "/var/lib/pacman"

		db_path = os.path.join(alpm_folder, "sync")

		# Build list of database names
		db_files = list(os.listdir(db_path)) if os.path.exists(db_path) else []
		self.db_names = [os.path.basename(db).split(".")[0] for db in db_files]

		# Get pyalpm handle
		alpm_handle = pyalpm.Handle("/", alpm_folder)

		self.pkg_objects = []

		# Build dictionary of names,install reasons of local packages
		local_db = alpm_handle.get_localdb()
		local_dict = dict([(pkg.name, pkg) for pkg in local_db.pkgcache])

		# Build list of PkgOBjects from packages in databases
		for db in self.db_names:
			sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

			if sync_db is not None:
				self.pkg_objects.extend([PkgObject(pkg) for pkg in sync_db.pkgcache])

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
app = LauncherApp(application_id="com.github.PkgView")
app.run(sys.argv)
