use std::collections::HashMap;
use taffy::prelude::*;

use crate::scene::{
    AlignItems, Display, ElementKind, FlexDirection, FlexWrap, JustifyContent, LayoutRect, NodeId,
    Overflow, Position, ResolvedStyle, SceneGraph, SceneNode, SizeValue,
};

pub struct LayoutEngine {
    tree: TaffyTree,
    node_map: HashMap<NodeId, taffy::NodeId>,
    reverse_map: HashMap<taffy::NodeId, NodeId>,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            tree: TaffyTree::new(),
            node_map: HashMap::new(),
            reverse_map: HashMap::new(),
        }
    }

    pub fn compute(&mut self, scene: &mut SceneGraph, viewport_width: f32, viewport_height: f32) {
        self.tree = TaffyTree::new();
        self.node_map.clear();
        self.reverse_map.clear();

        if let Some(root_id) = scene.root {
            // Force html and body nodes to fill viewport (like browsers do)
            for i in 0..scene.nodes.len() {
                let tag = scene.nodes[i].tag.clone();
                if tag == "html" || tag == "body" {
                    if scene.nodes[i].style.width == SizeValue::Auto {
                        scene.nodes[i].style.width = SizeValue::Px(viewport_width);
                    }
                    if scene.nodes[i].style.height == SizeValue::Auto {
                        scene.nodes[i].style.min_height = SizeValue::Px(viewport_height);
                    }
                    // Body scrolls by default when content overflows
                    if tag == "body" && scene.nodes[i].style.overflow == Overflow::Visible {
                        scene.nodes[i].style.overflow = Overflow::Scroll;
                    }
                }
            }

            let taffy_root = self.build_taffy_tree(scene, root_id, viewport_width, viewport_height);

            self.tree
                .compute_layout(
                    taffy_root,
                    Size {
                        width: AvailableSpace::Definite(viewport_width),
                        height: AvailableSpace::Definite(viewport_height),
                    },
                )
                .ok();

            self.apply_layout(scene, root_id, 0.0, 0.0);
        }
    }

    fn build_taffy_tree(
        &mut self,
        scene: &SceneGraph,
        node_id: NodeId,
        vw: f32,
        vh: f32,
    ) -> taffy::NodeId {
        let node = scene.get(node_id);
        let style = &node.style;

        let taffy_style = self.convert_style(style, node, vw, vh, scene);

        let children: Vec<taffy::NodeId> = node
            .children
            .iter()
            .map(|&child_id| self.build_taffy_tree(scene, child_id, vw, vh))
            .collect();

        let taffy_node = self.tree.new_with_children(taffy_style, &children).unwrap();
        self.node_map.insert(node_id, taffy_node);
        self.reverse_map.insert(taffy_node, node_id);
        taffy_node
    }

    fn convert_style(
        &self,
        style: &ResolvedStyle,
        node: &SceneNode,
        vw: f32,
        vh: f32,
        scene_graph: &SceneGraph,
    ) -> Style {
        self.build_taffy_style(style, node, vw, vh, scene_graph)
    }

    #[allow(clippy::field_reassign_with_default)]
    fn build_taffy_style(
        &self,
        style: &ResolvedStyle,
        node: &SceneNode,
        vw: f32,
        vh: f32,
        scene_graph: &SceneGraph,
    ) -> Style {
        let mut ts = Style::default();

        // Display
        ts.display = match style.display {
            Display::Flex => taffy::Display::Flex,
            Display::Block => taffy::Display::Block,
            Display::None => taffy::Display::None,
            Display::Inline => taffy::Display::Flex, // approximate inline as flex
        };

        // Flex properties
        ts.flex_direction = match style.flex_direction {
            FlexDirection::Row => taffy::FlexDirection::Row,
            FlexDirection::Column => taffy::FlexDirection::Column,
            FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        };

        ts.justify_content = Some(match style.justify_content {
            JustifyContent::Start => taffy::JustifyContent::FlexStart,
            JustifyContent::Center => taffy::JustifyContent::Center,
            JustifyContent::End => taffy::JustifyContent::FlexEnd,
            JustifyContent::SpaceBetween => taffy::JustifyContent::SpaceBetween,
            JustifyContent::SpaceAround => taffy::JustifyContent::SpaceAround,
            JustifyContent::SpaceEvenly => taffy::JustifyContent::SpaceEvenly,
        });

        ts.align_items = Some(match style.align_items {
            AlignItems::Stretch => taffy::AlignItems::Stretch,
            AlignItems::Start => taffy::AlignItems::FlexStart,
            AlignItems::Center => taffy::AlignItems::Center,
            AlignItems::End => taffy::AlignItems::FlexEnd,
        });

        ts.flex_wrap = match style.flex_wrap {
            FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
            FlexWrap::Wrap => taffy::FlexWrap::Wrap,
        };

        ts.flex_grow = style.flex_grow;
        ts.flex_shrink = style.flex_shrink;
        ts.gap = Size {
            width: LengthPercentage::Length(style.gap),
            height: LengthPercentage::Length(style.gap),
        };

        // Position
        ts.position = match style.position {
            Position::Relative => taffy::Position::Relative,
            Position::Absolute | Position::Fixed => taffy::Position::Absolute,
        };

        ts.inset = Rect {
            top: self.convert_length_auto(style.top, vw, vh),
            right: self.convert_length_auto(style.right, vw, vh),
            bottom: self.convert_length_auto(style.bottom, vw, vh),
            left: self.convert_length_auto(style.left, vw, vh),
        };

        // Size
        ts.size = Size {
            width: self.convert_dimension(style.font_size, style.width, vw, vh),
            height: self.convert_dimension(style.font_size, style.height, vw, vh),
        };
        ts.min_size = Size {
            width: self.convert_dimension(style.font_size, style.min_width, vw, vh),
            height: self.convert_dimension(style.font_size, style.min_height, vw, vh),
        };
        ts.max_size = Size {
            width: self.convert_dimension(style.font_size, style.max_width, vw, vh),
            height: self.convert_dimension(style.font_size, style.max_height, vw, vh),
        };

        // Margin
        ts.margin = Rect {
            top: LengthPercentageAuto::Length(style.margin[0]),
            right: LengthPercentageAuto::Length(style.margin[1]),
            bottom: LengthPercentageAuto::Length(style.margin[2]),
            left: LengthPercentageAuto::Length(style.margin[3]),
        };

        // Padding
        ts.padding = Rect {
            top: LengthPercentage::Length(style.padding[0]),
            right: LengthPercentage::Length(style.padding[1]),
            bottom: LengthPercentage::Length(style.padding[2]),
            left: LengthPercentage::Length(style.padding[3]),
        };

        // Border
        ts.border = Rect {
            top: LengthPercentage::Length(style.border_width[0]),
            right: LengthPercentage::Length(style.border_width[1]),
            bottom: LengthPercentage::Length(style.border_width[2]),
            left: LengthPercentage::Length(style.border_width[3]),
        };

        // Text nodes: inherit font properties from parent for sizing
        if node.kind == ElementKind::Text {
            if let Some(text) = &node.text_content {
                // Use parent's font size since CSS is applied to the container, not the text node
                let font_size = if let Some(parent_id) = node.parent {
                    scene_graph.get(parent_id).style.font_size
                } else {
                    style.font_size
                };
                let font_weight = if let Some(parent_id) = node.parent {
                    scene_graph.get(parent_id).style.font_weight
                } else {
                    style.font_weight
                };
                let line_height = if let Some(parent_id) = node.parent {
                    scene_graph.get(parent_id).style.line_height
                } else {
                    style.line_height
                };

                let char_count = text.chars().count() as f32;
                let ratio = if font_weight >= 700 { 0.72 } else { 0.62 };
                let text_width = char_count * font_size * ratio;
                let text_height = font_size * line_height;
                ts.size = Size {
                    width: Dimension::Length(text_width),
                    height: Dimension::Length(text_height),
                };
            }
        }

        // Block display: default to column layout for children
        if style.display == Display::Block {
            ts.display = taffy::Display::Flex;
            ts.flex_direction = taffy::FlexDirection::Column;
        }

        ts
    }

    fn convert_dimension(&self, font_size: f32, val: SizeValue, vw: f32, vh: f32) -> Dimension {
        match val {
            SizeValue::Px(v) => Dimension::Length(v),
            SizeValue::Percent(v) => Dimension::Percent(v / 100.0),
            SizeValue::Em(v) => Dimension::Length(v * font_size),
            SizeValue::Rem(v) => Dimension::Length(v * 16.0),
            SizeValue::Vh(v) => Dimension::Length(v / 100.0 * vh),
            SizeValue::Vw(v) => Dimension::Length(v / 100.0 * vw),
            SizeValue::Auto => Dimension::Auto,
        }
    }

    fn convert_length_auto(&self, val: SizeValue, vw: f32, vh: f32) -> LengthPercentageAuto {
        match val {
            SizeValue::Px(v) => LengthPercentageAuto::Length(v),
            SizeValue::Percent(v) => LengthPercentageAuto::Percent(v / 100.0),
            SizeValue::Vh(v) => LengthPercentageAuto::Length(v / 100.0 * vh),
            SizeValue::Vw(v) => LengthPercentageAuto::Length(v / 100.0 * vw),
            _ => LengthPercentageAuto::Auto,
        }
    }

    fn apply_layout(&self, scene: &mut SceneGraph, node_id: NodeId, parent_x: f32, parent_y: f32) {
        if let Some(&taffy_node) = self.node_map.get(&node_id) {
            if let Ok(layout) = self.tree.layout(taffy_node) {
                let x = parent_x + layout.location.x;
                let y = parent_y + layout.location.y;

                let scene_node = scene.get_mut(node_id);
                scene_node.layout = LayoutRect {
                    x,
                    y,
                    width: layout.size.width,
                    height: layout.size.height,
                };
                scene_node.dirty = false;

                let children: Vec<NodeId> = scene_node.children.clone();
                for child_id in children {
                    self.apply_layout(scene, child_id, x, y);
                }

                // Compute content_height from children (for scroll)
                let mut max_bottom: f32 = 0.0;
                for &child_id in &scene.get(node_id).children {
                    let child = scene.get(child_id);
                    let child_bottom = child.layout.y + child.layout.height - y;
                    if child_bottom > max_bottom {
                        max_bottom = child_bottom;
                    }
                }
                scene.get_mut(node_id).content_height = max_bottom;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Color;

    /// Build a scene with a single root element of the given tag.
    fn scene_with_root(tag: &str) -> (SceneGraph, NodeId) {
        let mut scene = SceneGraph::new();
        let root = scene.add_node(ElementKind::from_tag(tag), tag.to_string());
        (scene, root)
    }

    fn child(scene: &mut SceneGraph, parent: NodeId, tag: &str) -> NodeId {
        let id = scene.add_node(ElementKind::from_tag(tag), tag.to_string());
        scene.add_child(parent, id);
        id
    }

    fn text_child(scene: &mut SceneGraph, parent: NodeId, text: &str) -> NodeId {
        let id = scene.add_node(ElementKind::Text, "#text".to_string());
        scene.get_mut(id).text_content = Some(text.to_string());
        scene.add_child(parent, id);
        id
    }

    #[test]
    fn flex_row_children_are_laid_out_side_by_side() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.flex_direction = FlexDirection::Row;
        scene.get_mut(root).style.width = SizeValue::Px(300.0);
        scene.get_mut(root).style.height = SizeValue::Px(100.0);

        let a = child(&mut scene, root, "div");
        let b = child(&mut scene, root, "div");
        for id in [a, b] {
            scene.get_mut(id).style.width = SizeValue::Px(50.0);
            scene.get_mut(id).style.height = SizeValue::Px(20.0);
        }

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        assert_eq!(scene.get(root).layout.width, 300.0);
        assert_eq!(scene.get(a).layout.x, 0.0);
        assert_eq!(scene.get(b).layout.x, 50.0);
        assert_eq!(scene.get(a).layout.y, scene.get(b).layout.y);
        assert_eq!(scene.get(a).layout.width, 50.0);
    }

    #[test]
    fn flex_gap_separates_children() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.width = SizeValue::Px(300.0);
        scene.get_mut(root).style.gap = 12.0;

        let a = child(&mut scene, root, "div");
        let b = child(&mut scene, root, "div");
        for id in [a, b] {
            scene.get_mut(id).style.width = SizeValue::Px(50.0);
            scene.get_mut(id).style.height = SizeValue::Px(20.0);
        }

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        assert_eq!(scene.get(b).layout.x - scene.get(a).layout.x, 62.0);
    }

    #[test]
    fn nested_flex_positions_are_absolute_not_relative() {
        // outer (padding 10) > middle (column, margin-top 5) > inner
        let (mut scene, outer) = scene_with_root("div");
        scene.get_mut(outer).style.display = Display::Flex;
        scene.get_mut(outer).style.width = SizeValue::Px(400.0);
        scene.get_mut(outer).style.height = SizeValue::Px(300.0);
        scene.get_mut(outer).style.padding = [10.0, 10.0, 10.0, 10.0];

        let middle = child(&mut scene, outer, "div");
        scene.get_mut(middle).style.display = Display::Flex;
        scene.get_mut(middle).style.flex_direction = FlexDirection::Column;
        scene.get_mut(middle).style.width = SizeValue::Px(200.0);
        scene.get_mut(middle).style.height = SizeValue::Px(200.0);
        scene.get_mut(middle).style.margin = [5.0, 0.0, 0.0, 0.0];

        let inner = child(&mut scene, middle, "div");
        scene.get_mut(inner).style.width = SizeValue::Px(30.0);
        scene.get_mut(inner).style.height = SizeValue::Px(30.0);

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        // middle sits inside the padding box of outer, offset by its own top margin
        assert_eq!(scene.get(middle).layout.x, 10.0);
        assert_eq!(scene.get(middle).layout.y, 15.0);
        // inner coordinates are accumulated through both ancestors
        assert_eq!(scene.get(inner).layout.x, 10.0);
        assert_eq!(scene.get(inner).layout.y, 15.0);
        assert_eq!(scene.get(inner).layout.width, 30.0);
    }

    #[test]
    fn padding_shrinks_the_content_box_of_a_grown_child() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.width = SizeValue::Px(200.0);
        scene.get_mut(root).style.height = SizeValue::Px(100.0);
        scene.get_mut(root).style.padding = [10.0, 20.0, 10.0, 20.0];

        let inner = child(&mut scene, root, "div");
        scene.get_mut(inner).style.flex_grow = 1.0;

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        let l = &scene.get(inner).layout;
        assert_eq!(l.x, 20.0);
        assert_eq!(l.y, 10.0);
        assert_eq!(l.width, 160.0); // 200 - 20 - 20
        assert_eq!(l.height, 80.0); // 100 - 10 - 10 (align-items: stretch)
    }

    #[test]
    fn margins_offset_a_child_on_every_side() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.width = SizeValue::Px(200.0);
        scene.get_mut(root).style.height = SizeValue::Px(200.0);

        let inner = child(&mut scene, root, "div");
        // top, right, bottom, left
        scene.get_mut(inner).style.margin = [7.0, 11.0, 13.0, 17.0];
        scene.get_mut(inner).style.flex_grow = 1.0;

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        let l = &scene.get(inner).layout;
        assert_eq!(l.x, 17.0);
        assert_eq!(l.y, 7.0);
        assert_eq!(l.width, 200.0 - 17.0 - 11.0);
        assert_eq!(l.height, 200.0 - 7.0 - 13.0);
    }

    #[test]
    fn border_width_is_part_of_the_box() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.width = SizeValue::Px(100.0);
        scene.get_mut(root).style.height = SizeValue::Px(100.0);
        scene.get_mut(root).style.border_width = [4.0; 4];
        scene.get_mut(root).style.border_color = Color::BLACK;

        let inner = child(&mut scene, root, "div");
        scene.get_mut(inner).style.flex_grow = 1.0;

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        // border-box: the outer node keeps its declared size, the child is inset
        assert_eq!(scene.get(root).layout.width, 100.0);
        assert_eq!(scene.get(inner).layout.x, 4.0);
        assert_eq!(scene.get(inner).layout.width, 92.0);
    }

    #[test]
    fn percent_em_rem_and_viewport_units_resolve() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.width = SizeValue::Px(400.0);
        scene.get_mut(root).style.height = SizeValue::Px(400.0);

        let pct = child(&mut scene, root, "div");
        scene.get_mut(pct).style.width = SizeValue::Percent(25.0);
        scene.get_mut(pct).style.height = SizeValue::Px(10.0);

        let em = child(&mut scene, root, "div");
        scene.get_mut(em).style.font_size = 20.0;
        scene.get_mut(em).style.width = SizeValue::Em(2.0);
        scene.get_mut(em).style.height = SizeValue::Px(10.0);

        let rem = child(&mut scene, root, "div");
        scene.get_mut(rem).style.font_size = 20.0; // rem must ignore local font-size
        scene.get_mut(rem).style.width = SizeValue::Rem(2.0);
        scene.get_mut(rem).style.height = SizeValue::Px(10.0);

        let vw = child(&mut scene, root, "div");
        scene.get_mut(vw).style.width = SizeValue::Vw(50.0);
        scene.get_mut(vw).style.height = SizeValue::Vh(10.0);

        // no shrinking: assert the resolved sizes, not the flex fallout
        for id in [pct, em, rem, vw] {
            scene.get_mut(id).style.flex_shrink = 0.0;
        }

        LayoutEngine::new().compute(&mut scene, 1000.0, 600.0);

        assert_eq!(scene.get(pct).layout.width, 100.0); // 25% of 400
        assert_eq!(scene.get(em).layout.width, 40.0); // 2em at 20px
        assert_eq!(scene.get(rem).layout.width, 32.0); // 2rem at the 16px root size
        assert_eq!(scene.get(vw).layout.width, 500.0); // 50vw of 1000
        assert_eq!(scene.get(vw).layout.height, 60.0); // 10vh of 600
    }

    #[test]
    fn absolute_positioning_uses_inset() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.width = SizeValue::Px(300.0);
        scene.get_mut(root).style.height = SizeValue::Px(300.0);

        let abs = child(&mut scene, root, "div");
        scene.get_mut(abs).style.position = Position::Absolute;
        scene.get_mut(abs).style.top = SizeValue::Px(25.0);
        scene.get_mut(abs).style.left = SizeValue::Px(40.0);
        scene.get_mut(abs).style.width = SizeValue::Px(50.0);
        scene.get_mut(abs).style.height = SizeValue::Px(50.0);

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        assert_eq!(scene.get(abs).layout.x, 40.0);
        assert_eq!(scene.get(abs).layout.y, 25.0);
    }

    #[test]
    fn display_none_collapses_the_box() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.width = SizeValue::Px(200.0);
        scene.get_mut(root).style.height = SizeValue::Px(200.0);

        let hidden = child(&mut scene, root, "div");
        scene.get_mut(hidden).style.display = Display::None;
        scene.get_mut(hidden).style.width = SizeValue::Px(80.0);
        scene.get_mut(hidden).style.height = SizeValue::Px(80.0);

        let shown = child(&mut scene, root, "div");
        scene.get_mut(shown).style.width = SizeValue::Px(80.0);
        scene.get_mut(shown).style.height = SizeValue::Px(80.0);

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        assert_eq!(scene.get(hidden).layout.width, 0.0);
        assert_eq!(scene.get(hidden).layout.height, 0.0);
        // the hidden box takes no space in the main axis
        assert_eq!(scene.get(shown).layout.x, 0.0);
    }

    #[test]
    fn block_display_stacks_children_vertically() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Block;
        scene.get_mut(root).style.width = SizeValue::Px(200.0);
        scene.get_mut(root).style.height = SizeValue::Px(200.0);

        let a = child(&mut scene, root, "div");
        let b = child(&mut scene, root, "div");
        for id in [a, b] {
            scene.get_mut(id).style.width = SizeValue::Px(50.0);
            scene.get_mut(id).style.height = SizeValue::Px(30.0);
        }

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        assert_eq!(scene.get(a).layout.y, 0.0);
        assert_eq!(scene.get(b).layout.y, 30.0);
        assert_eq!(scene.get(a).layout.x, scene.get(b).layout.x);
    }

    #[test]
    fn text_nodes_are_measured_from_the_parent_font() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.width = SizeValue::Px(1000.0);
        scene.get_mut(root).style.height = SizeValue::Px(200.0);

        let p = child(&mut scene, root, "p");
        scene.get_mut(p).style.font_size = 20.0;
        scene.get_mut(p).style.line_height = 1.5;
        let t = text_child(&mut scene, p, "hello"); // 5 chars

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        let l = &scene.get(t).layout;
        // the regular-weight advance estimate is 0.62em per character
        assert!(
            (l.width - 5.0 * 20.0 * 0.62).abs() < 0.01,
            "width {}",
            l.width
        );
        assert!((l.height - 20.0 * 1.5).abs() < 0.01, "height {}", l.height);
    }

    #[test]
    fn bold_text_is_measured_wider_than_regular_text() {
        let mut widths = Vec::new();
        for weight in [400u16, 700u16] {
            let (mut scene, root) = scene_with_root("div");
            scene.get_mut(root).style.display = Display::Flex;
            scene.get_mut(root).style.width = SizeValue::Px(1000.0);
            let p = child(&mut scene, root, "p");
            scene.get_mut(p).style.font_weight = weight;
            let t = text_child(&mut scene, p, "hello world");
            LayoutEngine::new().compute(&mut scene, 800.0, 600.0);
            widths.push(scene.get(t).layout.width);
        }
        assert!(widths[1] > widths[0], "{:?}", widths);
    }

    #[test]
    fn html_and_body_are_stretched_to_the_viewport() {
        let mut scene = SceneGraph::new();
        let html = scene.add_node(ElementKind::from_tag("html"), "html".to_string());
        let body = child(&mut scene, html, "body");

        LayoutEngine::new().compute(&mut scene, 1024.0, 768.0);

        assert_eq!(scene.get(html).layout.width, 1024.0);
        assert_eq!(scene.get(body).layout.width, 1024.0);
        assert!(scene.get(html).layout.height >= 768.0);
        // body becomes scrollable so overflowing content can be reached
        assert_eq!(scene.get(body).style.overflow, Overflow::Scroll);
    }

    #[test]
    fn explicit_html_size_is_not_overridden() {
        let mut scene = SceneGraph::new();
        let html = scene.add_node(ElementKind::from_tag("html"), "html".to_string());
        scene.get_mut(html).style.width = SizeValue::Px(300.0);

        LayoutEngine::new().compute(&mut scene, 1024.0, 768.0);

        assert_eq!(scene.get(html).layout.width, 300.0);
    }

    #[test]
    fn content_height_tracks_overflowing_children() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.flex_direction = FlexDirection::Column;
        scene.get_mut(root).style.width = SizeValue::Px(200.0);
        scene.get_mut(root).style.height = SizeValue::Px(100.0);
        scene.get_mut(root).style.overflow = Overflow::Scroll;

        for _ in 0..4 {
            let c = child(&mut scene, root, "div");
            scene.get_mut(c).style.height = SizeValue::Px(60.0);
            scene.get_mut(c).style.flex_shrink = 0.0;
        }

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        assert_eq!(scene.get(root).layout.height, 100.0);
        assert_eq!(scene.get(root).content_height, 240.0);
    }

    #[test]
    fn min_and_max_size_constraints_are_applied() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.width = SizeValue::Px(400.0);
        scene.get_mut(root).style.height = SizeValue::Px(400.0);

        let clamped = child(&mut scene, root, "div");
        scene.get_mut(clamped).style.width = SizeValue::Px(300.0);
        scene.get_mut(clamped).style.max_width = SizeValue::Px(120.0);
        scene.get_mut(clamped).style.height = SizeValue::Px(10.0);
        scene.get_mut(clamped).style.min_height = SizeValue::Px(50.0);

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        assert_eq!(scene.get(clamped).layout.width, 120.0);
        assert_eq!(scene.get(clamped).layout.height, 50.0);
    }

    #[test]
    fn justify_content_center_centres_the_child() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.display = Display::Flex;
        scene.get_mut(root).style.justify_content = JustifyContent::Center;
        scene.get_mut(root).style.align_items = AlignItems::Center;
        scene.get_mut(root).style.width = SizeValue::Px(200.0);
        scene.get_mut(root).style.height = SizeValue::Px(200.0);

        let inner = child(&mut scene, root, "div");
        scene.get_mut(inner).style.width = SizeValue::Px(40.0);
        scene.get_mut(inner).style.height = SizeValue::Px(40.0);

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        assert_eq!(scene.get(inner).layout.x, 80.0);
        assert_eq!(scene.get(inner).layout.y, 80.0);
    }

    #[test]
    fn layout_clears_the_dirty_flag() {
        let (mut scene, root) = scene_with_root("div");
        scene.get_mut(root).style.width = SizeValue::Px(10.0);
        scene.get_mut(root).style.height = SizeValue::Px(10.0);
        scene.get_mut(root).dirty = true;

        LayoutEngine::new().compute(&mut scene, 800.0, 600.0);

        assert!(!scene.get(root).dirty);
    }
}
