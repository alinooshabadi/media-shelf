use eframe::egui::{self, RichText};
use media_shelf::{
    Category, Source, Video,
    file_ops::{self, OperationPlan},
    storage::{LibraryDb, database_path},
};
use std::sync::mpsc;

pub struct FileTools {
    dialog: Option<Dialog>,
    receiver: Option<mpsc::Receiver<Result<Response, String>>>,
}
enum Dialog {
    Preview {
        plan: OperationPlan,
        chosen: Vec<bool>,
        undo: Option<i64>,
    },
    Message(String),
}
enum Response {
    Renamed,
    Preview(OperationPlan, Option<i64>),
    Finished(String),
}

impl FileTools {
    pub fn new() -> Self {
        Self {
            dialog: None,
            receiver: None,
        }
    }
    pub fn active(&self) -> bool {
        self.dialog.is_some() || self.receiver.is_some()
    }
    pub fn rename(&mut self, video: Video, sources: Vec<Source>) {
        self.batch(vec![video], sources);
    }
    pub fn batch(&mut self, videos: Vec<Video>, sources: Vec<Source>) {
        self.spawn(move || {
            if videos.iter().any(|v| v.tmdb.is_none() && !media_shelf::nfo::read(v).ok().flatten().is_some_and(|n| n.year.is_some())) {
                return Err("No saved match or local NFO with title and year. Search & scrape those items before renaming.".into());
            }
            let requested: Vec<_> = videos
                .into_iter()
                .map(|v| {
                    let name = file_ops::suggested_name(&v);
                    (v, name)
                })
                .collect();
            let plan =
                file_ops::organize_preview(&sources, &requested).map_err(|e| e.to_string())?;
            if !plan.moves.is_empty() {
                let mut db = LibraryDb::open(&database_path()).map_err(|e| e.to_string())?;
                file_ops::apply(&mut db, &plan).map_err(|e| e.to_string())?;
            }
            Ok(Response::Renamed)
        });
    }
    pub fn cleanup(&mut self, sources: Vec<Source>, category: Category) {
        self.spawn(move || {
            Ok(Response::Preview(
                file_ops::cleanup_preview(&sources, category).map_err(|e| e.to_string())?,
                None,
            ))
        });
    }
    pub fn undo(&mut self) {
        self.spawn(move || {
            let db = LibraryDb::open(&database_path()).map_err(|e| e.to_string())?;
            let Some((id, plan)) = file_ops::last_undo(&db).map_err(|e| e.to_string())? else {
                return Ok(Response::Finished("No file operations to undo.".into()));
            };
            Ok(Response::Preview(plan, Some(id)))
        });
    }
    fn spawn(&mut self, task: impl FnOnce() -> Result<Response, String> + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.dialog = None;
        std::thread::spawn(move || {
            let _ = sender.send(task());
        });
    }
    pub fn draw(&mut self, ctx: &egui::Context) -> bool {
        let mut reload = false;
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.receiver = None;
                    self.dialog = Some(match result {
                        Ok(Response::Renamed) => {
                            self.dialog = None;
                            return true;
                        }
                        Ok(Response::Preview(plan, undo)) => {
                            let chosen =
                                vec![undo.is_some() || plan.kind == "rename"; plan.moves.len()];
                            Dialog::Preview { plan, chosen, undo }
                        }
                        Ok(Response::Finished(message)) => {
                            reload = true;
                            Dialog::Message(message)
                        }
                        Err(error) => {
                            reload = true;
                            Dialog::Message(format!("Operation stopped: {error}"))
                        }
                    });
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.dialog = Some(Dialog::Message(
                        "Worker stopped. Use Undo / recover last operation if any files moved."
                            .into(),
                    ));
                    reload = true;
                }
            }
        }
        if self.receiver.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            egui::Window::new("File operation")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Working…");
                    });
                });
            return reload;
        }
        let Some(dialog) = &mut self.dialog else {
            return reload;
        };
        let mut open = true;

        let mut apply_request = None;
        egui::Window::new("Rename & cleanup").id(egui::Id::new("file_tools"))
            .open(&mut open).default_width(760.0).default_height(480.0).show(ctx,|ui|{
                match dialog{
                    Dialog::Preview{plan,chosen,undo}=>{
                        let restoring=undo.is_some();
                        ui.heading(if restoring{"Undo / recover file operation"}else if plan.kind=="cleanup"{"Cleanup preview"}else{"Rename preview"});
                        if plan.kind=="cleanup"&&!restoring{
                            ui.label("Select leftovers to move into .mediashelf-cleanup. Undo restores them to their original paths.");
                            ui.label("Candidates: Thumbs.db, desktop.ini, .DS_Store, .sfv and .nzb files.");
                        }
                        if !restoring{ui.horizontal(|ui|{
                            if ui.button("Select all").clicked(){chosen.fill(true);}
                            if ui.button("Select none").clicked(){chosen.fill(false);}
                        });}
                        let count=chosen.iter().filter(|c|**c).count();
                        ui.label(format!("{count} of {} file changes selected",plan.moves.len()));
                        egui::ScrollArea::vertical().id_salt("file_moves").max_height(330.0).show(ui,|ui|{
                            for (index,item) in plan.moves.iter().enumerate(){
                                ui.horizontal(|ui|{
                                    ui.add_enabled(!restoring,egui::Checkbox::without_text(&mut chosen[index]));
                                    ui.vertical(|ui|{
                                        let (from,to)=if restoring{(&item.to,&item.from)}else{(&item.from,&item.to)};
                                        ui.label(RichText::new(from.display().to_string()).small());
                                        ui.label(RichText::new(format!("→ {}",to.display())).strong());
                                    });
                                });ui.separator();
                            }
                            for note in &plan.notes{ui.label(RichText::new(note).small().weak());}
                        });
                        if plan.moves.is_empty(){ui.label("No file changes found.");}
                        ui.separator();
                        if ui.add_enabled(count>0,egui::Button::new(if restoring{"Restore original paths"}else if plan.kind=="cleanup"{"Quarantine selected files"}else{"Apply selected renames"})).clicked(){
                            let mut selected=plan.clone();
                            if !restoring{selected.moves=selected.moves.into_iter().zip(chosen.iter()).filter_map(|(m,c)|if *c{Some(m)}else{None}).collect();}
                            apply_request=Some((selected,*undo));
                        }
                    },
                    Dialog::Message(message)=>{ui.label(message.as_str());},
                }
            });
        if !open {
            self.dialog = None;
        } else if let Some((plan, undo)) = apply_request {
            self.spawn(move||{
                let mut db=LibraryDb::open(&database_path()).map_err(|e|e.to_string())?;
                if let Some(id)=undo{file_ops::undo(&mut db,id,&plan).map_err(|e|e.to_string())?;Ok(Response::Finished(format!("Restored operation #{id}. Original paths and the library were updated.")))}
                else{let id=file_ops::apply(&mut db,&plan).map_err(|e|e.to_string())?;Ok(Response::Finished(format!("Applied {} file changes (operation #{id}). Undo is available from Organize files.",plan.moves.len())))}
            });
        }
        reload
    }
}
