use super::super::connectivity::client_server_message::*;
use super::super::connectivity::server_client_message::*;
use super::super::connectivity::controllable_object_message_data::*;
use super::unique_object_storage::*;
use std::sync::*;
use crossbeam_channel::TryRecvError;
use super::super::shared_types::*;
use super::server_viewport::*;
use super::unique_object::*;
use euclid::*;
use super::collision_component::*;
use super::shape::*;

pub struct PlayerObjectBinding {
    incoming_messages: crossbeam_channel::Receiver<ClientServerMessage>,
    outgoing_messages: crossbeam_channel::Sender<ServerClientMessage>,
    currently_controlled_object: Option<IdType>,
    controllable_objects: Arc<UniqueObjectStorage>,
    player_viewport_id: IdType,
    forward_already_pressed: bool,
    reverse_already_pressed: bool,
    left_already_pressed: bool,
    right_already_pressed: bool,
}

impl PlayerObjectBinding {
    pub fn new(incoming: crossbeam_channel::Receiver<ClientServerMessage>, outgoing: crossbeam_channel::Sender<ServerClientMessage>,  list: Arc<UniqueObjectStorage>, player_viewport_id: IdType) -> PlayerObjectBinding {
        PlayerObjectBinding{
            incoming_messages: incoming,
            outgoing_messages: outgoing,
            currently_controlled_object: None,
            controllable_objects: list,
            player_viewport_id: player_viewport_id,
            forward_already_pressed: false,
            reverse_already_pressed: false,
            left_already_pressed: false,
            right_already_pressed: false}
    }
    pub fn handle_updates(&mut self, delta_t: DeltaT) {

        match self.currently_controlled_object {
            Some(_has) => (),
            None => {
                let list = self.controllable_objects.all_objects();
                match list.iter().find(|x|
                    match x.get_type() {
                        ObjectType::Ship(_ignored) => true,
                        _ => false
                    }
                ) {
                    Some(exists) => {
                        let id = exists.get_id();
                        self.currently_controlled_object = Some(id);
                        self.outgoing_messages.send(ServerClientMessage::AssignControllableObject(AssignControllableObjectData{id: id}));
                    },
                    None => ()
                }
            }
        };


        loop {
            let message = match self.incoming_messages.try_recv() {
                Ok(has) => has,
                Err(TryRecvError::Empty) => {break;}
                Err(TryRecvError::Disconnected) => {panic!("Server disconnected")}
            };

            match message {
                ClientServerMessage::ControllableObjectMotionActionForward(data) => {
                    if !data.end_event {
                        if !self.reverse_already_pressed
                        {
                            self.forward_already_pressed = true;
                        }
                    } else {
                        self.forward_already_pressed = false;
                    }
                },
                ClientServerMessage::ControllableObjectMotionActionReverse(data) => {
                    if !data.end_event {
                        self.left_already_pressed = false;
                        self.right_already_pressed = false;
                        self.forward_already_pressed = false;
            
                        self.reverse_already_pressed = true;
                    } else {
                        self.reverse_already_pressed = false;
                    }
                },
                ClientServerMessage::ControllableObjectMotionActionLeft(data) => {
                    if !data.end_event {
                        if !self.right_already_pressed
                        {
                            self.left_already_pressed = true;
                        }
                    } else {
                        self.left_already_pressed = false;
                    }
                },
                ClientServerMessage::ControllableObjectMotionActionRight(data) => {
                    if !data.end_event {
                        if !self.left_already_pressed
                        {
                            self.right_already_pressed = true;
                        }
                    } else {
                        self.right_already_pressed = false;
                    }
                }
            };
        }

        let controlled: Option<std::sync::Arc<dyn UniqueObject + std::marker::Send + std::marker::Sync>> = match self.currently_controlled_object {
            Some(has) => {
                match self.controllable_objects.get_by_id(has) {
                    Some(object_exists) => {
                        match object_exists.get_type() {
                            ObjectType::Ship(_ship) => Some(object_exists),
                            _ => {
                                self.currently_controlled_object = None;
                                None
                            }
                        }
                    },
                    None => {
                        self.currently_controlled_object = None;
                        None
                    }
                }
            },
            None => None
        };

        match controlled {
            Some(has) => {
                match has.as_motion_component() {
                    Some(motion) => {
                        let component = motion.get_motion_component();

                        match self.controllable_objects.get_by_id(self.player_viewport_id) {
                            Some(player_viewport) => {
                                match player_viewport.as_collision_component() {
                                    Some(viewport_collision_component) => {
                                        match viewport_collision_component.get_shape() {
                                            Shape::Circle(data) => {
                                                let new_shape = Shape::Circle(CircleData{location: component.get_coordinates().location, radius: data.radius});
                                                viewport_collision_component.set_shape(new_shape);
                                            },
                                            _ => {panic!("Why the heck is the viewport not a circle?")}
                                        };
                                    },
                                    None => {
                                        panic!("Viewport doesn't have a collision component?");
                                    }
                                }
                            }, None => {
                                panic!("Player viewport ded");
                            }
                        }

                        let mut rotational_velocity: f32 = 0.0;
                        if self.left_already_pressed {
                            rotational_velocity = -1.0;
                        }

                        if self.right_already_pressed {
                            rotational_velocity = 1.0;
                        }

                        component.set_velocity(None, None, Some(rotational_velocity));

                        if self.forward_already_pressed {
                            component.apply_acceleration_along_pointing_direction(delta_t, 60.0);
                        }

                        if self.reverse_already_pressed {
                            let coordinates = component.get_coordinates();
                            let direction_of_velocity = coordinates.velocity.angle_from_x_axis();
                            let direction_to_reverse = direction_of_velocity + Angle::pi();
                            let direction_to_rotate = direction_to_reverse.angle_to(coordinates.rotation);
                            let float_angle = direction_to_rotate.to_degrees();
                            if float_angle.abs() < 1.0 {
                                component.apply_acceleration_along_pointing_direction(delta_t, 60.0);
                            } else {
                                let mut rotational_velocity: f32 = 0.0;
                                if float_angle < 0.0 {
                                    rotational_velocity = -1.0;
                                }
        
                                if float_angle > 0.0 {
                                    rotational_velocity = 1.0;
                                }

                                component.set_velocity(None, None, Some(rotational_velocity));
                            }
                        }
                    },
                    None => {}
                }
            },
            _ => {}
        }
    }
}