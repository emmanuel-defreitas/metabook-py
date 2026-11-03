//! Retained document phases: switching tabs never recreates an explorer.
use super::{MetabookApp, Phase};
use ely_gpui_component::navigation::{EditorTab, EditorTabs};
use gpui::{Context, IntoElement, SharedString};

pub(super) fn book_tab_id(id: &str, gutenberg_id: Option<u64>) -> SharedString {
    gutenberg_id
        .map_or_else(|| format!("upload-{id}"), |id| format!("gutenberg-{id}"))
        .into()
}

pub(super) struct Workspace {
    pub active: SharedString,
    pub title: SharedString,
    pub pending: Option<SharedString>,
    pub next: Option<SharedString>,
    parked: Vec<(SharedString, SharedString, Phase)>,
    order: Vec<SharedString>,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            active: "home".into(),
            title: "Home".into(),
            pending: None,
            next: None,
            parked: Vec::new(),
            order: vec!["home".into()],
        }
    }
}
impl Workspace {
    fn select(&mut self, phase: &mut Phase, id: &SharedString) -> bool {
        if self.active == *id {
            return false;
        }
        let Some(index) = self.parked.iter().position(|tab| tab.0 == *id) else {
            return false;
        };
        let (id, title, next) = self.parked.remove(index);
        self.parked.push((
            self.active.clone(),
            self.title.clone(),
            std::mem::replace(phase, next),
        ));
        self.active = id;
        self.title = title;
        true
    }
}
impl MetabookApp {
    pub(super) fn reopen_workspace_tab(
        &mut self,
        id: &SharedString,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.workspace.active == *id || self.workspace.parked.iter().any(|tab| tab.0 == *id) {
            self.select_workspace_tab(id, cx);
            true
        } else {
            false
        }
    }
    pub(super) fn select_workspace_tab(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if self.workspace.select(&mut self.phase, id) {
            cx.notify();
        }
    }

    pub(super) fn open_request_tab(&mut self, cx: &mut Context<Self>) {
        let id = self
            .workspace
            .next
            .take()
            .unwrap_or_else(|| format!("request-{}", self.request_ix + 1).into());
        if !self.reopen_workspace_tab(&id, cx) {
            self.workspace.parked.push((
                self.workspace.active.clone(),
                self.workspace.title.clone(),
                std::mem::replace(&mut self.phase, Phase::Idle),
            ));
            self.workspace.active = id.clone();
            self.workspace.order.push(id.clone());
        }
        self.workspace.title = "Loading…".into();
        self.workspace.pending = Some(id);
    }
    fn close_workspace_tab(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if id.as_ref() == "home" {
            return;
        }
        if self.workspace.pending.as_ref() == Some(id) {
            // Discard a late response rather than reopening a dismissed tab.
            self.request_ix += 1;
            self.workspace.pending = None;
            self.upload_progress = None;
            self.query
                .update(cx, |query, cx| query.set_disabled(false, cx));
        }
        if self.workspace.active == *id {
            self.select_workspace_tab(&"home".into(), cx);
        }
        self.workspace.parked.retain(|tab| tab.0 != *id);
        self.workspace.order.retain(|tab| tab != id);
        cx.notify();
    }
    pub(super) fn render_workspace_tabs(&self, cx: &Context<Self>) -> impl IntoElement {
        let mut tabs: Vec<_> = self
            .workspace
            .parked
            .iter()
            .map(|tab| (tab.0.clone(), tab.1.clone()))
            .collect();
        tabs.push((self.workspace.active.clone(), self.workspace.title.clone()));
        tabs.sort_by_key(|tab| self.workspace.order.iter().position(|id| id == &tab.0));
        let owner = cx.entity();
        EditorTabs::new(
            "workspace-tabs",
            tabs.into_iter().map(|(id, title)| {
                let home = id.as_ref() == "home";
                EditorTab::new(id, title).pinned(home)
            }),
        )
        .selected(self.workspace.active.clone())
        .on_select(cx.listener(|this, id, _, cx| this.select_workspace_tab(id, cx)))
        .on_close(cx.listener(|this, id, _, cx| this.close_workspace_tab(id, cx)))
        .on_reorder(move |from, to, _, cx| {
            owner.update(cx, |this, cx| {
                if from == 0
                    || to == 0
                    || from >= this.workspace.order.len()
                    || to >= this.workspace.order.len()
                {
                    return;
                }
                let id = this.workspace.order.remove(from);
                this.workspace.order.insert(to, id);
                cx.notify();
            });
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_catalog_book_uses_same_tab_identity_as_search() {
        assert_eq!(
            book_tab_id("saved-row", Some(1342)).as_ref(),
            "gutenberg-1342"
        );
        assert_eq!(book_tab_id("epub-row", None).as_ref(), "upload-epub-row");
    }
    #[test]
    fn switching_keeps_documents_and_home_independent() {
        let mut workspace = Workspace::default();
        workspace.parked.push((
            "book".into(),
            "Book".into(),
            Phase::Failed {
                message: "book state".into(),
            },
        ));
        let mut phase = Phase::Idle;
        assert!(workspace.select(&mut phase, &"book".into()));
        assert!(matches!(&phase, Phase::Failed { message } if message.as_ref() == "book state"));
        assert!(workspace.select(&mut phase, &"home".into()));
        assert!(matches!(phase, Phase::Idle));
        assert!(workspace.select(&mut phase, &"book".into()));
        assert!(matches!(&phase, Phase::Failed { message } if message.as_ref() == "book state"));
        assert_eq!(workspace.parked.len(), 1);
    }
    #[test]
    fn completing_background_tab_does_not_replace_home() {
        let mut workspace = Workspace::default();
        workspace.parked.push((
            "book".into(),
            "Loading".into(),
            Phase::Processing {
                message: "loading".into(),
                detail_loading: true,
                home_view: false,
            },
        ));
        let mut phase = Phase::Idle;
        workspace.pending = Some("book".into());
        let original = workspace.active.clone();
        let pending = workspace.pending.clone().unwrap();
        workspace.select(&mut phase, &pending);
        phase = Phase::Failed {
            message: "completed response".into(),
        };
        workspace.pending = None;
        workspace.select(&mut phase, &original);
        assert!(matches!(phase, Phase::Idle));
        assert_eq!(workspace.active.as_ref(), "home");
        workspace.select(&mut phase, &"book".into());
        assert!(
            matches!(&phase, Phase::Failed { message } if message.as_ref() == "completed response")
        );
    }
}
