use crate::node_atlas::NodeAtlas;
use crate::quadtree::{NodeData, Nodes, Quadtree, TreeUpdate, Viewer};
use crate::{
    AssetEvent, AssetServer, Camera, EventReader, GlobalTransform, Image, QuadtreeUpdate, Query,
    Res, ViewDistance, With,
};
use bevy::math::Vec3Swizzles;
use std::mem;

/// Traverses all quadtrees and generates a new tree update.
pub fn traverse_quadtree(
    viewer_query: Query<(&GlobalTransform, &ViewDistance), With<Camera>>,
    mut terrain_query: Query<(&GlobalTransform, &mut Quadtree, &mut TreeUpdate)>,
) {
    for (terrain_transform, mut quadtree, mut tree_update) in terrain_query.iter_mut() {
        for (camera_transform, view_distance) in viewer_query.iter() {
            let viewer = Viewer {
                position: (camera_transform.translation - terrain_transform.translation).xz(),
                view_distance: view_distance.view_distance,
            };

            quadtree.traverse(&mut tree_update, viewer);
        }
    }
}

/// Updates the nodes and the node atlas according to the corresponding tree update
/// and the load statuses.
pub fn update_nodes(
    asset_server: Res<AssetServer>,
    mut terrain_query: Query<(
        &mut TreeUpdate,
        &mut Nodes,
        &mut NodeAtlas,
        &mut QuadtreeUpdate,
    )>,
) {
    for (mut tree_update, mut nodes, mut node_atlas, mut node_updates) in terrain_query.iter_mut() {
        let Nodes {
            ref mut handle_mapping,
            ref mut load_statuses,
            ref mut loading_nodes,
            ref mut inactive_nodes,
            ref mut active_nodes,
        } = nodes.as_mut();

        // clear the previously activated nodes
        tree_update.activated_nodes.clear();

        let mut nodes_to_activate: Vec<NodeData> = Vec::new();

        // load required nodes from cache or disk
        for id in mem::take(&mut tree_update.nodes_to_activate) {
            if let Some(node) = inactive_nodes.pop(&id) {
                // queue cached node for activation
                nodes_to_activate.push(node);
            } else {
                // load node before activation
                loading_nodes.insert(
                    id,
                    NodeData::load(id, &asset_server, load_statuses, handle_mapping),
                );
            };
        }

        // queue all nodes that have finished loading for activation
        load_statuses.retain(|&id, status| {
            if status.finished {
                nodes_to_activate.push(loading_nodes.remove(&id).unwrap());
            }

            !status.finished
        });

        // deactivate all no longer required nodes
        for id in mem::take(&mut tree_update.nodes_to_deactivate) {
            let mut node = active_nodes.remove(&id).unwrap();
            node_atlas.remove_node(&mut node, &mut node_updates.0);
            inactive_nodes.put(id, node);
        }

        // activate as many nodes as there are available atlas ids
        for mut node in nodes_to_activate {
            node_atlas.add_node(&mut node, &mut node_updates.0);
            tree_update.activated_nodes.insert(node.id);
            active_nodes.insert(node.id, node);
        }
    }
}

/// Updates the load status of a node for all of it newly loaded assets.
pub fn update_load_status(
    mut asset_events: EventReader<AssetEvent<Image>>,
    mut terrain_query: Query<&mut Nodes>,
) {
    for event in asset_events.iter() {
        if let AssetEvent::Created { handle } = event {
            for mut nodes in terrain_query.iter_mut() {
                if let Some(id) = nodes.handle_mapping.remove(&handle.id) {
                    let status = nodes.load_statuses.get_mut(&id).unwrap();
                    status.finished = true;
                    break;
                }
            }
        }
    }
}