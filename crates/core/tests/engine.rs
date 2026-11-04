use std::{fs, path::Path, sync::Arc, time::Duration};

use crownos_files_core::{
    fs::{
        engine::{EngineCmd, run_engine},
        mounts::MountTable,
    },
    model::{
        files_op::FilesOp,
        folder::{Folder, Stage},
        view_options::ViewOptions,
    },
};
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, unbounded_channel},
        watch,
    },
    time::{error::Elapsed, timeout},
};

const PATIENCE: Duration = Duration::from_secs(20);
const QUIET: Duration = Duration::from_millis(400);

async fn load_into(
    folder: &mut Folder,
    ops: &mut UnboundedReceiver<FilesOp>,
) -> Result<(), Elapsed> {
    let applied = async {
        while let Some(op) = ops.recv().await {
            let done = matches!(op, FilesOp::Done { .. });
            folder.apply(op);
            if done {
                return;
            }
        }
    };
    timeout(PATIENCE, applied).await
}

#[tokio::test]
async fn loads_revalidates_and_measures_through_commands() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("photos/2024")).unwrap();
    fs::write(temp.path().join("photos/a.jpg"), vec![0; 1000]).unwrap();
    fs::write(temp.path().join("photos/2024/b.jpg"), vec![0; 234]).unwrap();
    fs::write(temp.path().join("notes.md"), b"# hi").unwrap();
    let dir: Arc<Path> = temp.path().into();

    let (ops_sender, mut ops) = unbounded_channel();
    let (commands, receiver) = unbounded_channel();
    let (_mounts, table) = watch::channel(Arc::new(MountTable::read().unwrap()));
    let engine = tokio::spawn(run_engine(ops_sender, receiver, table));

    let mut folder = Folder::new(dir.clone(), ViewOptions::default());
    commands.send(EngineCmd::Load(dir.clone())).unwrap();
    load_into(&mut folder, &mut ops).await.unwrap();
    assert_eq!(folder.stage(), Stage::Loaded);
    let names: Vec<_> = folder.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, ["photos", "notes.md"]);

    commands
        .send(EngineCmd::Revalidate {
            dir: dir.clone(),
            stamp: folder.stamp(),
        })
        .unwrap();
    assert!(
        timeout(QUIET, ops.recv()).await.is_err(),
        "a fresh folder was listed again"
    );

    fs::write(temp.path().join("new.txt"), b"").unwrap();
    commands
        .send(EngineCmd::Revalidate {
            dir: dir.clone(),
            stamp: folder.stamp(),
        })
        .unwrap();
    load_into(&mut folder, &mut ops).await.unwrap();
    assert_eq!(folder.len(), 3);

    commands
        .send(EngineCmd::MeasureDirs {
            dir: dir.clone(),
            names: vec!["photos".into()],
        })
        .unwrap();
    let measured = timeout(PATIENCE, ops.recv()).await.unwrap().unwrap();
    folder.apply(measured);
    assert_eq!(folder.get(0).map(|entry| entry.stat.len), Some(1234));

    drop(commands);
    timeout(PATIENCE, engine).await.unwrap().unwrap().unwrap();
}
