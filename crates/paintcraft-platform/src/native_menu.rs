//! Main-thread macOS menus. Hosts supply semantic IDs and revalidate actions.
use muda::{
    CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, accelerator::Accelerator,
};
use std::{collections::HashMap, sync::mpsc};

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u64,
    pub text: String,
    pub shortcut: Option<&'static str>,
    pub checked: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub title: String,
    pub entries: Vec<Entry>,
}

enum Item {
    Normal(MenuItem),
    Check(CheckMenuItem),
}

pub struct NativeMenu {
    menu: Menu,
    items: HashMap<u64, Item>,
    events: mpsc::Receiver<u64>,
}

impl std::fmt::Debug for NativeMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeMenu")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl NativeMenu {
    pub fn new(
        sections: Vec<Section>,
        settings_id: u64,
        quit_id: u64,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let menu = Menu::new();
        let mut items = HashMap::new();
        let app = Submenu::new("PaintCraft", true);
        app.append(&PredefinedMenuItem::about(
            Some("About PaintCraft"),
            Some(muda::AboutMetadata {
                name: Some("PaintCraft".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                ..Default::default()
            }),
        ))
        .map_err(|e| e.to_string())?;
        app.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        append(
            &app,
            &mut items,
            Entry {
                id: settings_id,
                text: "Settings…".into(),
                shortcut: Some("meta+Comma"),
                checked: None,
            },
        )?;
        app.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        for item in [
            PredefinedMenuItem::hide(None),
            PredefinedMenuItem::hide_others(None),
            PredefinedMenuItem::show_all(None),
        ] {
            app.append(&item).map_err(|e| e.to_string())?;
        }
        app.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        append(
            &app,
            &mut items,
            Entry {
                id: quit_id,
                text: "Quit PaintCraft".into(),
                shortcut: Some("meta+KeyQ"),
                checked: None,
            },
        )?;
        menu.append(&app).map_err(|e| e.to_string())?;
        for section in sections {
            let submenu = Submenu::new(section.title, true);
            for entry in section.entries {
                append(&submenu, &mut items, entry)?;
            }
            menu.append(&submenu).map_err(|e| e.to_string())?;
        }
        let window = Submenu::new("Window", true);
        window
            .append(&PredefinedMenuItem::minimize(None))
            .map_err(|e| e.to_string())?;
        window
            .append(&PredefinedMenuItem::maximize(None))
            .map_err(|e| e.to_string())?;
        menu.append(&window).map_err(|e| e.to_string())?;
        window.set_as_windows_menu_for_nsapp();
        let (sender, events) = mpsc::channel();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some(id) = event
                .id
                .0
                .strip_prefix("paintcraft-native-")
                .and_then(|id| id.parse::<u64>().ok())
            {
                let _ = sender.send(id);
                wake();
            }
        }));
        menu.init_for_nsapp();
        Ok(Self {
            menu,
            items,
            events,
        })
    }

    pub fn poll(&self) -> Vec<u64> {
        self.events.try_iter().collect()
    }

    pub fn update(&self, id: u64, enabled: bool, checked: Option<bool>) {
        match self.items.get(&id) {
            Some(Item::Normal(item)) => item.set_enabled(enabled),
            Some(Item::Check(item)) => {
                item.set_enabled(enabled);
                if let Some(value) = checked {
                    item.set_checked(value);
                }
            }
            None => {}
        }
    }
}

fn append(menu: &Submenu, items: &mut HashMap<u64, Item>, entry: Entry) -> Result<(), String> {
    let accelerator = entry
        .shortcut
        .map(str::parse::<Accelerator>)
        .transpose()
        .map_err(|e| e.to_string())?;
    let id = format!("paintcraft-native-{}", entry.id);
    let item = if let Some(checked) = entry.checked {
        let item = CheckMenuItem::with_id(id, entry.text, true, checked, accelerator);
        menu.append(&item).map_err(|e| e.to_string())?;
        Item::Check(item)
    } else {
        let item = MenuItem::with_id(id, entry.text, true, accelerator);
        menu.append(&item).map_err(|e| e.to_string())?;
        Item::Normal(item)
    };
    items.insert(entry.id, item);
    Ok(())
}

impl Drop for NativeMenu {
    fn drop(&mut self) {
        self.menu.remove_for_nsapp();
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    }
}
