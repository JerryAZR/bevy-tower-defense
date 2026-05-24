use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // Spawn a 2D camera
    commands.spawn(Camera2d);

    // Spawn a red square in the center of the screen
    commands.spawn(Sprite::from_color(
        Color::srgb(1.0, 0.0, 0.0),
        Vec2::new(100.0, 100.0),
    ));
}
