use crate::scene::*;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Click(f32, f32),
    MouseMove(f32, f32),
    KeyPress(String),
    Scroll(f32, f32, f32, f32), // x, y, dx, dy
}

#[derive(Debug, Clone)]
pub struct EventResult {
    pub target: NodeId,
    pub event_type: String,
    pub handler_code: String,
}

pub struct EventSystem {
    pub hover_node: Option<NodeId>,
    pub focus_node: Option<NodeId>,
}

impl Default for EventSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSystem {
    pub fn new() -> Self {
        Self {
            hover_node: None,
            focus_node: None,
        }
    }

    pub fn handle(&mut self, event: &AppEvent, scene: &mut SceneGraph) -> Vec<EventResult> {
        match event {
            AppEvent::Click(x, y) => self.handle_click(*x, *y, scene),
            AppEvent::MouseMove(x, y) => self.handle_mouse_move(*x, *y, scene),
            AppEvent::Scroll(x, y, dx, dy) => {
                self.handle_scroll(*x, *y, *dx, *dy, scene);
                Vec::new()
            }
            AppEvent::KeyPress(key) => self.handle_key(key, scene),
        }
    }

    fn handle_click(&mut self, x: f32, y: f32, scene: &SceneGraph) -> Vec<EventResult> {
        let mut results = Vec::new();

        if let Some(hit) = self.hit_test(x, y, scene) {
            // Walk up from hit node, collecting click handlers (bubble)
            let mut current = Some(hit);
            while let Some(node_id) = current {
                let node = scene.get(node_id);
                if let Some(handler) = node.event_handlers.get("click") {
                    results.push(EventResult {
                        target: hit,
                        event_type: "click".to_string(),
                        handler_code: handler.clone(),
                    });
                }
                current = node.parent;
            }
        }

        results
    }

    fn handle_mouse_move(&mut self, x: f32, y: f32, scene: &mut SceneGraph) -> Vec<EventResult> {
        let mut results = Vec::new();
        let new_hover = self.hit_test(x, y, scene);

        if new_hover != self.hover_node {
            // Remove hover style from old node
            if let Some(old) = self.hover_node {
                let node = scene.get(old);
                let handler = node.event_handlers.get("mouseleave").cloned();
                let has_base = node.base_style.is_some();
                let has_transition = node.style.transition_duration > 0.0;
                if let Some(handler) = handler {
                    results.push(EventResult {
                        target: old,
                        event_type: "mouseleave".to_string(),
                        handler_code: handler,
                    });
                }
                if has_base {
                    let base = scene.get(old).base_style.clone().unwrap();
                    if has_transition {
                        // Start transition back to base
                        let current = scene.get(old).style.clone();
                        let node = scene.get_mut(old);
                        node.transition_from = Some(current);
                        node.transition_to = Some(base);
                        node.transition_start = Some(std::time::Instant::now());
                    } else {
                        scene.get_mut(old).style = base;
                    }
                    scene.get_mut(old).is_hovered = false;
                    scene.get_mut(old).dirty = true;
                }
            }
            // Apply hover style to new node
            if let Some(new) = new_hover {
                let node = scene.get(new);
                let handler = node.event_handlers.get("mouseenter").cloned();
                let has_hover = node.hover_style.is_some();
                let has_transition = node.style.transition_duration > 0.0
                    || node
                        .hover_style
                        .as_ref()
                        .map(|h| h.transition_duration > 0.0)
                        .unwrap_or(false);
                if let Some(handler) = handler {
                    results.push(EventResult {
                        target: new,
                        event_type: "mouseenter".to_string(),
                        handler_code: handler,
                    });
                }
                if has_hover {
                    let hover = scene.get(new).hover_style.clone().unwrap();
                    if has_transition {
                        // Start transition to hover style
                        let current = scene.get(new).style.clone();
                        let node = scene.get_mut(new);
                        node.transition_from = Some(current);
                        node.transition_to = Some(hover);
                        node.transition_start = Some(std::time::Instant::now());
                    } else {
                        scene.get_mut(new).style = hover;
                    }
                    scene.get_mut(new).is_hovered = true;
                    scene.get_mut(new).dirty = true;
                }
            }
            self.hover_node = new_hover;
        }

        results
    }

    fn handle_scroll(&mut self, x: f32, y: f32, _dx: f32, dy: f32, scene: &mut SceneGraph) {
        // Find the deepest scrollable container at (x, y)
        if let Some(root) = scene.root {
            if let Some(scroll_target) = self.find_scrollable(x, y, scene, root) {
                let node = scene.get_mut(scroll_target);
                let max_scroll = (node.content_height - node.layout.height).max(0.0);
                node.scroll_offset.1 = (node.scroll_offset.1 + dy * 30.0).clamp(0.0, max_scroll);
            }
        }
    }

    fn handle_key(&self, _key: &str, scene: &SceneGraph) -> Vec<EventResult> {
        let mut results = Vec::new();

        if let Some(focus) = self.focus_node {
            let node = scene.get(focus);
            if let Some(handler) = node.event_handlers.get("keypress") {
                results.push(EventResult {
                    target: focus,
                    event_type: "keypress".to_string(),
                    handler_code: handler.clone(),
                });
            }
        }

        results
    }

    fn hit_test(&self, x: f32, y: f32, scene: &SceneGraph) -> Option<NodeId> {
        if let Some(root) = scene.root {
            self.hit_test_recursive(x, y, scene, root)
        } else {
            None
        }
    }

    fn hit_test_recursive(
        &self,
        x: f32,
        y: f32,
        scene: &SceneGraph,
        node_id: NodeId,
    ) -> Option<NodeId> {
        let node = scene.get(node_id);

        if node.style.display == Display::None {
            return None;
        }

        let layout = &node.layout;
        // Account for scroll offset
        let scroll_y = node.scroll_offset.1;
        let in_bounds = x >= layout.x
            && x <= layout.x + layout.width
            && y >= layout.y
            && y <= layout.y + layout.height;

        if !in_bounds && node.style.overflow != Overflow::Visible {
            return None;
        }

        // Adjust child hit coords for this node's scroll offset
        let child_y = y + scroll_y;

        // Check children in reverse order (last drawn = on top)
        let children: Vec<NodeId> = node.children.clone();
        for &child_id in children.iter().rev() {
            if let Some(hit) = self.hit_test_recursive(x, child_y, scene, child_id) {
                return Some(hit);
            }
        }

        if in_bounds && !node.event_handlers.is_empty() {
            return Some(node_id);
        }

        if in_bounds {
            return Some(node_id);
        }

        None
    }

    fn find_scrollable(
        &self,
        x: f32,
        y: f32,
        scene: &SceneGraph,
        node_id: NodeId,
    ) -> Option<NodeId> {
        let node = scene.get(node_id);
        let layout = &node.layout;

        let in_bounds = x >= layout.x
            && x <= layout.x + layout.width
            && y >= layout.y
            && y <= layout.y + layout.height;

        if !in_bounds {
            return None;
        }

        // Check children first (deepest scrollable wins)
        let children: Vec<NodeId> = node.children.clone();
        for &child_id in children.iter().rev() {
            if let Some(found) = self.find_scrollable(x, y, scene, child_id) {
                return Some(found);
            }
        }

        if node.style.overflow == Overflow::Scroll {
            return Some(node_id);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(scene: &mut SceneGraph, tag: &str, rect: (f32, f32, f32, f32)) -> NodeId {
        let id = scene.add_node(ElementKind::from_tag(tag), tag.to_string());
        scene.get_mut(id).layout = LayoutRect {
            x: rect.0,
            y: rect.1,
            width: rect.2,
            height: rect.3,
        };
        id
    }

    fn on(scene: &mut SceneGraph, id: NodeId, event: &str, code: &str) {
        scene
            .get_mut(id)
            .event_handlers
            .insert(event.to_string(), code.to_string());
    }

    /// root 0,0 200x200 containing a 100x100 box at 50,50
    fn simple_scene() -> (SceneGraph, NodeId, NodeId) {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 200.0, 200.0));
        let inner = node(&mut scene, "div", (50.0, 50.0, 100.0, 100.0));
        scene.add_child(root, inner);
        (scene, root, inner)
    }

    #[test]
    fn click_inside_a_child_hits_the_child() {
        let (mut scene, _root, inner) = simple_scene();
        on(&mut scene, inner, "click", "inner()");

        let results = EventSystem::new().handle(&AppEvent::Click(100.0, 100.0), &mut scene);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].target, inner);
        assert_eq!(results[0].event_type, "click");
        assert_eq!(results[0].handler_code, "inner()");
    }

    #[test]
    fn click_outside_the_child_falls_through_to_the_parent() {
        let (mut scene, root, inner) = simple_scene();
        on(&mut scene, inner, "click", "inner()");
        on(&mut scene, root, "click", "root()");

        let results = EventSystem::new().handle(&AppEvent::Click(10.0, 10.0), &mut scene);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].target, root);
        assert_eq!(results[0].handler_code, "root()");
    }

    #[test]
    fn click_bubbles_from_the_target_up_through_its_ancestors() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 300.0, 300.0));
        let mid = node(&mut scene, "div", (0.0, 0.0, 200.0, 200.0));
        let leaf = node(&mut scene, "button", (0.0, 0.0, 100.0, 100.0));
        scene.add_child(root, mid);
        scene.add_child(mid, leaf);
        on(&mut scene, root, "click", "root()");
        on(&mut scene, mid, "click", "mid()");
        on(&mut scene, leaf, "click", "leaf()");

        let results = EventSystem::new().handle(&AppEvent::Click(10.0, 10.0), &mut scene);

        // innermost handler first, then each ancestor
        let codes: Vec<&str> = results.iter().map(|r| r.handler_code.as_str()).collect();
        assert_eq!(codes, vec!["leaf()", "mid()", "root()"]);
        // every result reports the hit node as the event target
        assert!(results.iter().all(|r| r.target == leaf));
    }

    #[test]
    fn a_click_that_misses_everything_produces_no_results() {
        let (mut scene, root, inner) = simple_scene();
        on(&mut scene, inner, "click", "inner()");
        on(&mut scene, root, "click", "root()");

        let results = EventSystem::new().handle(&AppEvent::Click(999.0, 999.0), &mut scene);

        assert!(results.is_empty());
    }

    #[test]
    fn the_last_of_two_overlapping_siblings_wins() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 200.0, 200.0));
        let under = node(&mut scene, "div", (0.0, 0.0, 100.0, 100.0));
        let over = node(&mut scene, "div", (0.0, 0.0, 100.0, 100.0));
        scene.add_child(root, under);
        scene.add_child(root, over);
        on(&mut scene, under, "click", "under()");
        on(&mut scene, over, "click", "over()");

        let results = EventSystem::new().handle(&AppEvent::Click(50.0, 50.0), &mut scene);

        assert_eq!(results[0].target, over, "later siblings paint on top");
        assert_eq!(results[0].handler_code, "over()");
    }

    #[test]
    fn display_none_subtrees_are_not_hit_testable() {
        let (mut scene, root, inner) = simple_scene();
        scene.get_mut(inner).style.display = Display::None;
        on(&mut scene, inner, "click", "inner()");
        on(&mut scene, root, "click", "root()");

        let results = EventSystem::new().handle(&AppEvent::Click(100.0, 100.0), &mut scene);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].target, root);
    }

    #[test]
    fn hit_testing_is_inclusive_of_the_box_edges() {
        let (mut scene, _root, inner) = simple_scene();
        on(&mut scene, inner, "click", "inner()");

        let mut sys = EventSystem::new();
        // top-left and bottom-right corners of the 50,50 100x100 box
        assert_eq!(
            sys.handle(&AppEvent::Click(50.0, 50.0), &mut scene).len(),
            1
        );
        assert_eq!(
            sys.handle(&AppEvent::Click(150.0, 150.0), &mut scene).len(),
            1
        );
        // one pixel past the right edge is the parent, which has no handler
        assert!(sys
            .handle(&AppEvent::Click(151.0, 150.0), &mut scene)
            .is_empty());
    }

    #[test]
    fn mouse_move_tracks_the_hovered_node() {
        let (mut scene, root, inner) = simple_scene();
        let mut sys = EventSystem::new();

        sys.handle(&AppEvent::MouseMove(100.0, 100.0), &mut scene);
        assert_eq!(sys.hover_node, Some(inner));

        sys.handle(&AppEvent::MouseMove(5.0, 5.0), &mut scene);
        assert_eq!(sys.hover_node, Some(root));

        sys.handle(&AppEvent::MouseMove(-10.0, -10.0), &mut scene);
        assert_eq!(sys.hover_node, None);
    }

    #[test]
    fn hover_style_is_applied_on_enter_and_restored_on_leave() {
        let (mut scene, _root, inner) = simple_scene();
        let base = scene.get(inner).style.clone();
        let mut hover = base.clone();
        hover.background_color = Color::from_rgba(255, 0, 0, 1.0);
        scene.get_mut(inner).base_style = Some(base.clone());
        scene.get_mut(inner).hover_style = Some(hover);

        let mut sys = EventSystem::new();
        sys.handle(&AppEvent::MouseMove(100.0, 100.0), &mut scene);
        assert!(scene.get(inner).is_hovered);
        assert_eq!(scene.get(inner).style.background_color.r, 1.0);

        sys.handle(&AppEvent::MouseMove(5.0, 5.0), &mut scene);
        assert!(!scene.get(inner).is_hovered);
        assert_eq!(
            scene.get(inner).style.background_color,
            base.background_color
        );
    }

    #[test]
    fn a_transition_defers_the_hover_style_instead_of_snapping() {
        let (mut scene, _root, inner) = simple_scene();
        let base = scene.get(inner).style.clone();
        let mut hover = base.clone();
        hover.background_color = Color::from_rgba(255, 0, 0, 1.0);
        hover.transition_duration = 0.3;
        scene.get_mut(inner).base_style = Some(base.clone());
        scene.get_mut(inner).hover_style = Some(hover);

        EventSystem::new().handle(&AppEvent::MouseMove(100.0, 100.0), &mut scene);

        let node = scene.get(inner);
        assert!(node.transition_start.is_some());
        assert!(node.transition_to.is_some());
        // the visible style is still the base one; tick_animations interpolates it
        assert_eq!(node.style.background_color, base.background_color);
    }

    #[test]
    fn mouseenter_and_mouseleave_handlers_fire_once_per_crossing() {
        let (mut scene, _root, inner) = simple_scene();
        on(&mut scene, inner, "mouseenter", "enter()");
        on(&mut scene, inner, "mouseleave", "leave()");
        let mut sys = EventSystem::new();

        let enter = sys.handle(&AppEvent::MouseMove(100.0, 100.0), &mut scene);
        assert_eq!(enter.len(), 1);
        assert_eq!(enter[0].event_type, "mouseenter");

        // moving within the same node fires nothing
        assert!(sys
            .handle(&AppEvent::MouseMove(110.0, 110.0), &mut scene)
            .is_empty());

        let leave = sys.handle(&AppEvent::MouseMove(5.0, 5.0), &mut scene);
        assert_eq!(leave.len(), 1);
        assert_eq!(leave[0].event_type, "mouseleave");
        assert_eq!(leave[0].target, inner);
    }

    #[test]
    fn scrolling_moves_a_scroll_container_and_clamps_at_both_ends() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 200.0, 100.0));
        scene.get_mut(root).style.overflow = Overflow::Scroll;
        scene.get_mut(root).content_height = 400.0;

        let mut sys = EventSystem::new();
        sys.handle(&AppEvent::Scroll(50.0, 50.0, 0.0, 2.0), &mut scene);
        assert_eq!(scene.get(root).scroll_offset.1, 60.0); // 2 lines * 30px

        // cannot scroll above the top
        sys.handle(&AppEvent::Scroll(50.0, 50.0, 0.0, -10.0), &mut scene);
        assert_eq!(scene.get(root).scroll_offset.1, 0.0);

        // cannot scroll past content_height - height
        sys.handle(&AppEvent::Scroll(50.0, 50.0, 0.0, 100.0), &mut scene);
        assert_eq!(scene.get(root).scroll_offset.1, 300.0);
    }

    #[test]
    fn scrolling_targets_the_innermost_scrollable_ancestor() {
        let mut scene = SceneGraph::new();
        let outer = node(&mut scene, "div", (0.0, 0.0, 200.0, 200.0));
        scene.get_mut(outer).style.overflow = Overflow::Scroll;
        scene.get_mut(outer).content_height = 1000.0;
        let inner = node(&mut scene, "div", (0.0, 0.0, 100.0, 100.0));
        scene.get_mut(inner).style.overflow = Overflow::Scroll;
        scene.get_mut(inner).content_height = 1000.0;
        scene.add_child(outer, inner);

        let mut sys = EventSystem::new();
        sys.handle(&AppEvent::Scroll(50.0, 50.0, 0.0, 1.0), &mut scene);

        assert_eq!(scene.get(inner).scroll_offset.1, 30.0);
        assert_eq!(scene.get(outer).scroll_offset.1, 0.0);

        // outside the inner box, the outer container scrolls instead
        sys.handle(&AppEvent::Scroll(150.0, 150.0, 0.0, 1.0), &mut scene);
        assert_eq!(scene.get(outer).scroll_offset.1, 30.0);
    }

    #[test]
    fn a_non_scrollable_container_ignores_the_wheel() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 200.0, 100.0));
        scene.get_mut(root).content_height = 400.0;

        EventSystem::new().handle(&AppEvent::Scroll(50.0, 50.0, 0.0, 3.0), &mut scene);

        assert_eq!(scene.get(root).scroll_offset.1, 0.0);
    }

    #[test]
    fn keypress_only_dispatches_to_the_focused_node() {
        let (mut scene, _root, inner) = simple_scene();
        on(&mut scene, inner, "keypress", "key()");
        let mut sys = EventSystem::new();

        assert!(sys
            .handle(&AppEvent::KeyPress("a".into()), &mut scene)
            .is_empty());

        sys.focus_node = Some(inner);
        let results = sys.handle(&AppEvent::KeyPress("a".into()), &mut scene);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].event_type, "keypress");
        assert_eq!(results[0].target, inner);
    }

    #[test]
    fn hit_testing_compensates_for_the_scroll_offset_of_a_container() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 200.0, 100.0));
        scene.get_mut(root).style.overflow = Overflow::Scroll;
        // a child that has been scrolled up out of the visible box
        let item = node(&mut scene, "div", (0.0, 120.0, 200.0, 40.0));
        scene.add_child(root, item);
        on(&mut scene, item, "click", "item()");

        let mut sys = EventSystem::new();
        // unscrolled: y=30 is inside the container but above the item
        assert!(sys
            .handle(&AppEvent::Click(10.0, 30.0), &mut scene)
            .is_empty());

        // scroll the container down so the item moves into view
        scene.get_mut(root).scroll_offset.1 = 100.0;
        let results = sys.handle(&AppEvent::Click(10.0, 30.0), &mut scene);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].target, item);
    }
}
