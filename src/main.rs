use {
    bevy::{
        DefaultPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin, Startup},
        ecs::{change_detection::ResMut, system::IsFunctionSystem},
        log::info,
    },
    bevy_tokio_tasks::{
        MainThreadContext, TaskContext, TokioTasksPlugin, TokioTasksRuntime,
        tokio::task::JoinHandle,
    },
    std::time::Duration,
};

const FRAMES_PER_SECOND: f32 = 30.0;

fn setup(runtime: ResMut<'_, TokioTasksRuntime>) {
    let _: JoinHandle<()> =
        runtime.spawn_background_task::<_, (), _>(|mut ctx: TaskContext| async move {
            () = ctx
                .run_on_main_thread::<_, ()>(|_ctx: MainThreadContext<'_>| {
                    info!("coi le munje");
                })
                .await;
        });
}

fn main() {
    let mut app: App = App::new();

    let _: &mut App = app
        .add_plugins::<_>(DefaultPlugins.set::<ScheduleRunnerPlugin>(
            ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(FRAMES_PER_SECOND.recip())),
        ))
        .add_plugins::<_>(TokioTasksPlugin::default())
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // ResMut<'_, TokioTasksRuntime>
            ),
        )>(Startup, setup);

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
