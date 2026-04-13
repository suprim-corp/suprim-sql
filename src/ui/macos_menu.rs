//! Native macOS menu bar integration.
//!
//! Replaces the default winit menu with Connection / Query / View menus
//! that appear next to the Apple logo in the system menu bar.
//!
//! Architecture: each `NSMenuItem` targets a small ObjC helper object
//! (`MenuHandler`) that writes a [`MenuAction`] to an `mpsc` channel.
//! The eframe `App::logic()` polls this channel every frame.

use std::sync::mpsc;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem, NSWindowButton};
use objc2_foundation::{MainThreadMarker, NSObject, NSString};

// ── Public types ────────────────────────────────────────────────────────

/// Actions that can be triggered from the native macOS menu bar.
#[derive(Debug, Clone)]
pub enum MenuAction {
    NewConnection,
    Quit,
    NewSqlTab,
    ReloadDatabases,
    QueryHistory,
    DataTransfer,
    DataGeneration,
    DataDictionary,
    DataSynchronization,
    StructureSynchronization,
}

// ── ObjC bridge ─────────────────────────────────────────────────────────

/// Per-menu-item ivars: which action to fire and where to send it.
struct MenuHandlerIvars {
    action: MenuAction,
    sender: mpsc::Sender<MenuAction>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SuprimMenuHandler"]
    #[ivars = MenuHandlerIvars]
    struct MenuHandler;

    impl MenuHandler {
        #[unsafe(method(handleAction:))]
        fn handle_action(&self, _sender: Option<&NSObject>) {
            let ivars = self.ivars();
            let _ = ivars.sender.send(ivars.action.clone());
        }
    }
);

impl MenuHandler {
    fn new(
        mtm: MainThreadMarker,
        action: MenuAction,
        sender: mpsc::Sender<MenuAction>,
    ) -> Retained<Self> {
        let this = mtm
            .alloc::<Self>()
            .set_ivars(MenuHandlerIvars { action, sender });
        unsafe { msg_send![super(this), init] }
    }
}

// ── Public API ──────────────────────────────────────────────────────────

/// Install result: the receiver end and the handler objects that must stay alive.
pub struct NativeMenu {
    pub rx: mpsc::Receiver<MenuAction>,
    /// Must be kept alive — dropping deallocates the ObjC targets and crashes.
    _handlers: Vec<Retained<MenuHandler>>,
}

/// Build and install the native macOS menu bar.
///
/// # Panics
/// Panics if not called from the main thread.
pub fn install_native_menu() -> NativeMenu {
    let mtm =
        MainThreadMarker::new().expect("install_native_menu must be called on the main thread");
    let app = NSApplication::sharedApplication(mtm);

    let (tx, rx) = mpsc::channel();
    let mut handlers: Vec<Retained<MenuHandler>> = Vec::new();

    let menubar = NSMenu::new(mtm);

    // ── App menu (SuprimSQL) ──
    let app_menu = build_submenu(
        mtm,
        "",
        &tx,
        &mut handlers,
        &[("Quit SuprimSQL", MenuAction::Quit, "q")],
    );
    menubar.addItem(&app_menu);

    // ── Connection menu ──
    let conn_menu = build_submenu(
        mtm,
        "Connection",
        &tx,
        &mut handlers,
        &[("New Connection\u{2026}", MenuAction::NewConnection, "n")],
    );
    menubar.addItem(&conn_menu);

    // ── Query menu ──
    let query_menu = build_submenu(
        mtm,
        "Query",
        &tx,
        &mut handlers,
        &[("New SQL Tab", MenuAction::NewSqlTab, "t")],
    );
    menubar.addItem(&query_menu);

    // ── View menu ──
    let view_menu = build_submenu(
        mtm,
        "View",
        &tx,
        &mut handlers,
        &[
            ("Reload Databases", MenuAction::ReloadDatabases, "r"),
            ("Query History", MenuAction::QueryHistory, "y"),
        ],
    );
    menubar.addItem(&view_menu);

    // ── Tools menu ──
    let tools_menu = build_submenu(
        mtm,
        "Tools",
        &tx,
        &mut handlers,
        &[
            ("Data Transfer\u{2026}", MenuAction::DataTransfer, "T"),
            ("Data Generation\u{2026}", MenuAction::DataGeneration, ""),
            ("Data Dictionary\u{2026}", MenuAction::DataDictionary, "D"),
            (
                "Data Synchronization\u{2026}",
                MenuAction::DataSynchronization,
                "",
            ),
            (
                "Structure Synchronization\u{2026}",
                MenuAction::StructureSynchronization,
                "",
            ),
        ],
    );
    menubar.addItem(&tools_menu);

    app.setMainMenu(Some(&menubar));

    NativeMenu {
        rx,
        _handlers: handlers,
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Create a top-level menu item with a submenu containing the given entries.
fn build_submenu(
    mtm: MainThreadMarker,
    title: &str,
    tx: &mpsc::Sender<MenuAction>,
    handlers: &mut Vec<Retained<MenuHandler>>,
    items: &[(&str, MenuAction, &str)],
) -> Retained<NSMenuItem> {
    let menu_item = NSMenuItem::new(mtm);
    let submenu = if title.is_empty() {
        NSMenu::new(mtm)
    } else {
        let ns_title = NSString::from_str(title);
        NSMenu::initWithTitle(mtm.alloc(), &ns_title)
    };

    for &(label, ref action, key) in items {
        let handler = MenuHandler::new(mtm, action.clone(), tx.clone());
        let item = make_item(mtm, label, sel!(handleAction:), &handler, key);
        submenu.addItem(&item);
        handlers.push(handler);
    }

    menu_item.setSubmenu(Some(&submenu));
    menu_item
}

/// Create a single `NSMenuItem` with a keyboard shortcut and target.
fn make_item(
    mtm: MainThreadMarker,
    title: &str,
    action: Sel,
    target: &MenuHandler,
    key_equivalent: &str,
) -> Retained<NSMenuItem> {
    let ns_title = NSString::from_str(title);
    let ns_key = NSString::from_str(key_equivalent);
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &ns_title,
            Some(action),
            &ns_key,
        )
    };
    unsafe { item.setTarget(Some(target.as_ref())) };
    item
}

/// Reposition macOS traffic-light buttons so they are vertically centered
/// within the custom title bar of the given height.
///
/// Called every frame because macOS layout resets the position.
pub fn center_traffic_lights(title_bar_height: f64) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(window) = app.mainWindow() else {
        return;
    };
    let Some(close_btn) = window.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    let Some(container) = (unsafe { close_btn.superview() }) else {
        return;
    };
    let Some(parent) = (unsafe { container.superview() }) else {
        return;
    };

    let cf = container.frame();
    let parent_h = parent.frame().size.height;
    let new_y = parent_h - title_bar_height + (title_bar_height - cf.size.height) / 2.0;

    if (new_y - cf.origin.y).abs() > 0.5 {
        container.setFrame(objc2_foundation::NSRect::new(
            objc2_foundation::NSPoint::new(cf.origin.x, new_y),
            cf.size,
        ));
    }
}
