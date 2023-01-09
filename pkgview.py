#!/usr/bin/env python

import gi, sys, os
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject
import pyalpm, datetime

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

	status = GObject.Property(type=int, default=-1)

	@GObject.Property(type=str, default="")
	def status_string(self):
		if self.status == -1: return("")

		if self.status == 0:
			return("explicit")
		else:
			if self.status == 1:
				return("dependency")
			else:
				return("optional" if self.status == 2 else "orphan")

	@GObject.Property(type=str, default="")
	def status_icon(self):
		if self.status == -1: return("")

		if self.status == 0:
			return("package-install")
		else:
			if self.status == 1:
				return("package-installed-updated")
			else:
				return("package-installed-outdated" if self.status == 2 else "package-purge")

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

	repo_filter = ""
	status_filter = []

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	sort_model = Gtk.Template.Child()
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
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind column factories to signals
		self.name_factory.connect("setup", self.on_item_setup_iconlabel)
		self.name_factory.connect("bind", self.on_item_bind_name)
		self.version_factory.connect("setup", self.on_item_setup_label)
		self.version_factory.connect("bind", self.on_item_bind_version)
		self.repository_factory.connect("setup", self.on_item_setup_label)
		self.repository_factory.connect("bind", self.on_item_bind_repository)
		self.status_factory.connect("setup", self.on_item_setup_iconlabel)
		self.status_factory.connect("bind", self.on_item_bind_status)
		self.date_factory.connect("setup", self.on_item_setup_label)
		self.date_factory.connect("bind", self.on_item_bind_date)
		self.size_factory.connect("setup", self.on_item_setup_label)
		self.size_factory.connect("bind", self.on_item_bind_size)

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
	def on_item_setup_label(self, factory, item):
		item.set_child(Gtk.Label(halign=Gtk.Align.START))

	def on_item_setup_iconlabel(self, factory, item):
		box = Gtk.Box()
		box.set_spacing(6)
		box.append(Gtk.Image())
		box.append(Gtk.Label(halign=Gtk.Align.START))
		item.set_child(box)

	def on_item_bind_name(self, factory, item):
		item.get_child().get_first_child().set_from_icon_name("package-x-generic-symbolic")
		item.get_child().get_last_child().set_label(item.get_item().name)

	def on_item_bind_version(self, factory, item):
		item.get_child().set_label(item.get_item().version)

	def on_item_bind_repository(self, factory, item):
		item.get_child().set_label(item.get_item().repository)

	def on_item_bind_status(self, factory, item):
		item.get_child().get_first_child().set_from_icon_name(item.get_item().status_icon)
		item.get_child().get_last_child().set_label(item.get_item().status_string)

	def on_item_bind_date(self, factory, item):
		item.get_child().set_label(item.get_item().date_string)

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
		match_status = True if self.status_filter == [] else (item.status in self.status_filter)
		return(match_repo and match_status)

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
	repolist_allrow = Gtk.Template.Child()

	status_listbox = Gtk.Template.Child()
	statuslist_installedrow = Gtk.Template.Child()

	pkg_columnview = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		for db in app.db_names:
			box = Gtk.Box(margin_start=6, margin_end=6, spacing=6)
			box.append(Gtk.Image(icon_name="package-x-generic-symbolic"))
			box.append(Gtk.Label(label=str.title(db)))

			self.repo_listbox.append(Gtk.ListBoxRow(child=box))

		self.repo_listbox.connect("row-selected", self.on_repo_changed)
		self.status_listbox.connect("row-selected", self.on_status_changed)

		self.repo_listbox.select_row(self.repolist_allrow)
		self.status_listbox.select_row(self.statuslist_installedrow)

		self.pkg_columnview.model.splice(0, 0, app.pkg_objects)

	def on_repo_changed(self, listbox, row):
		text = row.get_child().get_last_child().get_text().lower()

		self.pkg_columnview.repo_filter = "" if text == "all" else text

		self.pkg_columnview.pkg_filter.changed(Gtk.FilterChange.DIFFERENT)

	def on_status_changed(self, listbox, row):
		text = row.get_child().get_last_child().get_text().lower()

		match text:
			case "explicit": self.pkg_columnview.status_filter = [0]
			case "installed": self.pkg_columnview.status_filter = [0, 1, 2, 3]
			case "dependency": self.pkg_columnview.status_filter = [1]
			case "optional": self.pkg_columnview.status_filter = [2]
			case "orphan": self.pkg_columnview.status_filter = [3]
			case _: self.pkg_columnview.status_filter = []

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

				if reason == 0: self.pkg_objects[i].status = 0
				else:
					if reason == 1:
						if local_dict[obj.pkg.name].compute_requiredby() != []:
							self.pkg_objects[i].status = 1
						else:
							self.pkg_objects[i].status = 2 if local_dict[obj.pkg.name].compute_optionalfor() != [] else 3

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
