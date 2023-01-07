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
	pkg = None

	#-----------------------------------
	# Properties
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
	def status(self):
		if self.pkg.reason == 0:
			return("installed")
		else:
			if self.pkg.compute_requiredby() != []: return("dependency")
			else:
				return("optional" if self.pkg.compute_optionalfor() != [] else "orphan")

	@GObject.Property(type=str, default="")
	def sdate(self):
		return(str(datetime.datetime.fromtimestamp(self.pkg.installdate)))

	@GObject.Property(type=int, default=0)
	def date(self):
		return(self.pkg.installdate)

	@GObject.Property(type=str, default="")
	def ssize(self):
		pkg_size = self.pkg.isize

		for unit in ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB']:
			if pkg_size < 1024.0 or unit == 'PiB':
				break
			pkg_size /= 1024.0
		
		return(f"{pkg_size:.1f} {unit}")

	@GObject.Property(type=int, default=0)
	def size(self):
		return(self.pkg.isize)

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
	sort_model = Gtk.Template.Child()
	model = Gtk.Template.Child()

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

		# Bind column sorters to sort function
		self.name_sorter.set_sort_func(self.sort_by_name_column)
		self.version_sorter.set_sort_func(self.sort_by_version_column)
		self.repository_sorter.set_sort_func(self.sort_by_repository_column)
		self.status_sorter.set_sort_func(self.sort_by_status_column)
		self.date_sorter.set_sort_func(self.sort_by_date_column)
		self.size_sorter.set_sort_func(self.sort_by_size_column)

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
		# item.get_child().get_first_child().set_from_icon_name(icon)
		item.get_child().get_last_child().set_label(item.get_item().status)

	def on_item_bind_date(self, factory, item):
		item.get_child().set_label(item.get_item().sdate)

	def on_item_bind_size(self, factory, item):
		item.get_child().set_label(item.get_item().ssize)

	#-----------------------------------
	# Sorter functions
	#-----------------------------------
	def sort_by_name_column(self, item_a, item_b, user_data):
		if item_a.name < item_b.name: return(-1)
		else:
			if item_a.name > item_b.name: return(1)
			else: return(0)

	def sort_by_version_column(self, item_a, item_b, user_data):
		return(pyalpm.vercmp(item_a.version, item_b.version))

	def sort_by_repository_column(self, item_a, item_b, user_data):
		if item_a.repository < item_b.repository: return(-1)
		else:
			if item_a.repository > item_b.repository: return(1)
			else: return(0)

	def sort_by_status_column(self, item_a, item_b, user_data):
		status_a = item_a.status
		status_b = item_b.status

		if status_a < status_b: return(-1)
		else:
			if status_a > status_b: return(1)
			else: return(0)

	def sort_by_date_column(self, item_a, item_b, user_data):
		return(item_a.date - item_b.date)

	def sort_by_size_column(self, item_a, item_b, user_data):
		return(item_a.size - item_b.size)

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

		self.repo_listbox.select_row(self.repolist_allrow)
		self.status_listbox.select_row(self.statuslist_installedrow)

		self.pkg_columnview.model.splice(0, 0, app.pkg_objects)

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

		alpm_folder = "/var/lib/pacman"

		db_path = os.path.join(alpm_folder, "sync")

		db_files = list(os.listdir(db_path)) if os.path.exists(db_path) else []
		db_files = [os.path.basename(db).split(".")[0] for db in db_files]

		alpm_handle = pyalpm.Handle("/", alpm_folder)
		self.local_db = alpm_handle.get_localdb()

		self.pkg_objects = []

		# for db in db_files:
		# 	sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

		# 	if sync_db is not None:
		# 		self.pkg_objects.extend([PkgObject(pkg) for pkg in sync_db.pkgcache])

		self.pkg_objects.extend([PkgObject(pkg) for pkg in self.local_db.pkgcache])

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
