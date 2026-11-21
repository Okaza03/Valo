//! Admin › Upload: pick a movie or series folder on either disk, go into it
//! (or make a folder), and upload files there. Files go in chunks on a thread
//! of their own, so uploads carry on while the admin looks elsewhere; one that
//! stopped resumes when the same file is picked into the same folder again.
//! When a batch is done the server rescans, so the new titles show up.
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, space, text, text_input, Column};
use iced::{Background, Border, Element, Length, Padding};

use super::super::details::gap;
use super::super::profile::Remote;
use super::super::{spin_secs, Msg as LMsg};
use super::ui::{self, caps, mono, panel};
use super::{not_set_up, Cx, Done, JobFn, Msg as AMsg, Section};
use crate::api::{self, Chunk, Client, UploadFolders, UploadInfo, UploadRoot};
use crate::iced_ui::icon::{self, Icon};
use crate::iced_ui::style::{self, c};
use crate::iced_ui::widgets::Kind as Btn;
use crate::ui::browse::details::TEAL;
use crate::ui::theme::{CARD, ERROR, LINE, LINE_HI, OK, SURFACE_HI, WARN};

/// What can be uploaded (the server checks too).
const EXTENSIONS: [&str; 9] = [
    "mkv", "mp4", "avi", "mov", "srt", "vtt", "jpg", "jpeg", "png",
];
/// Folders listed before asking for a filter.
const SHOW: usize = 300;
/// A chunk when the server doesn't say, and the bounds of what it says.
const CHUNK: u64 = 8 << 20;
const CHUNK_MIN: u64 = 256 << 10;
const CHUNK_MAX: u64 = 64 << 20;
/// A chunk that fails (the connection dropped) is tried this many times.
const TRIES: u32 = 6;

#[derive(Clone, Debug)]
pub enum Msg {
    /// Show one of the server's movie or series folders (its id).
    Root(String),
    /// Go into a folder of the one shown.
    Open(String),
    /// Go back up to the breadcrumb `n` deep (0: the root).
    Up(usize),
    Filter(String),
    /// Open (`Some`) or close the new folder field, or type in it.
    NewFolder(Option<String>),
    MakeFolder,
    /// Choose files to upload into the folder shown.
    Pick,
    /// Stop an upload (its key).
    Cancel(u64),
    /// Take finished, failed and stopped uploads off the list.
    Clear,
    /// Delete an unfinished upload on the server (its id).
    Forget(String),
}

#[derive(Clone, Debug, PartialEq)]
enum Stage {
    Waiting,
    Sending,
    /// All sent: the server is moving it into place.
    Finishing,
    Done,
    Failed(String),
    Stopped,
}

/// A file on the list.
struct Entry {
    key: u64,
    local: PathBuf,
    name: String,
    size: u64,
    root: String,
    path: String,
    /// "Disk 1 · Movies / Heat (1995)".
    dest: String,
    /// The server's id, once it started.
    id: Option<String>,
    sent: u64,
    /// Where this run started and when, for the speed.
    from: u64,
    began: Option<Instant>,
    stage: Stage,
    stop: bool,
}

/// The upload list, shared with the thread sending it.
#[derive(Default)]
struct Queue {
    files: Vec<Entry>,
    next_key: u64,
    /// The thread is going.
    running: bool,
    /// Counts finished batches, so the page lists the folders again.
    batches: u64,
    /// The rescan after the last batch, and whether it worked.
    note: Option<(String, bool)>,
}

#[derive(Default)]
pub struct State {
    folders: Remote<UploadFolders>,
    /// The root and folder (path parts) to show.
    root: Option<String>,
    path: Vec<String>,
    filter: String,
    new_folder: Option<String>,
    unfinished: Remote<Vec<UploadInfo>>,
    queue: Arc<Mutex<Queue>>,
    /// The batch count last seen.
    batches: u64,
}

impl State {
    pub fn refresh(&mut self) {
        self.folders.retry();
        self.unfinished.retry();
    }

    pub fn missing(&self) -> bool {
        self.folders.missing
    }

    /// Uploads going or waiting, for the sidebar.
    pub fn active(&self) -> usize {
        let q = self.queue.lock().unwrap();
        q.files
            .iter()
            .filter(|e| matches!(e.stage, Stage::Waiting | Stage::Sending | Stage::Finishing))
            .count()
    }

    fn rel(&self) -> String {
        self.path.join("/")
    }

    pub fn work(&mut self, client: &Client, login: &mut bool) {
        let batches = self.queue.lock().unwrap().batches;
        if batches != self.batches {
            self.batches = batches;
            self.refresh();
        }
        if self.folders.poll(login) {
            // nothing picked yet: the first root
            if self.root.is_none() {
                let first = self.folders.data.as_ref().and_then(|f| f.roots.first());
                if let Some(r) = first {
                    self.root = Some(r.id.clone());
                }
            }
        }
        let want = (self.root.clone(), self.rel());
        let have = self
            .folders
            .data
            .as_ref()
            .map(|f| (f.root.clone(), f.path.clone()));
        let moved = have.is_some_and(|h| h != want) && self.folders.error.is_none();
        if self.folders.due() || (moved && !self.folders.loading()) {
            let c = client.clone();
            self.folders
                .load(move || c.admin_upload_folders(want.0.as_deref(), &want.1));
        }
        self.unfinished.poll(login);
        if self.unfinished.due() {
            let c = client.clone();
            self.unfinished.load(move || c.admin_uploads());
        }
    }

    fn root_info(&self) -> Option<&UploadRoot> {
        let f = self.folders.data.as_ref()?;
        f.roots.iter().find(|r| Some(&r.id) == self.root.as_ref())
    }

    fn label(&self, id: &str) -> String {
        self.folders
            .data
            .as_ref()
            .map_or_else(|| id.to_owned(), |f| f.label_by_id(id))
    }

    fn go(&mut self, root: Option<String>, path: Vec<String>) {
        self.root = root;
        self.path = path;
        self.filter.clear();
        self.new_folder = None;
        // a wrong place shouldn't stick
        self.folders.error = None;
    }

    pub fn update(&mut self, m: Msg, client: &Client) -> Option<JobFn> {
        match m {
            Msg::Root(id) => self.go(Some(id), Vec::new()),
            Msg::Open(name) => {
                let mut p = self.path.clone();
                p.push(name);
                self.go(self.root.clone(), p);
            }
            Msg::Up(n) => {
                let p = self.path[..n.min(self.path.len())].to_vec();
                self.go(self.root.clone(), p);
            }
            Msg::Filter(s) => self.filter = s,
            Msg::NewFolder(v) => self.new_folder = v,
            Msg::MakeFolder => {
                let name = self.new_folder.take()?.trim().to_owned();
                let root = self.root.clone()?;
                if name.is_empty() {
                    return None;
                }
                let rel = self.rel();
                return Some(Box::new(move |c: &Client| {
                    let path = c.admin_make_folder(&root, &rel, &name)?;
                    Ok(Done::msg(format!("Made the folder {path}.")))
                }));
            }
            Msg::Pick => self.pick(client),
            Msg::Cancel(key) => {
                let mut q = self.queue.lock().unwrap();
                if let Some(e) = q.files.iter_mut().find(|e| e.key == key) {
                    match e.stage {
                        Stage::Waiting => e.stage = Stage::Stopped,
                        Stage::Sending => e.stop = true,
                        _ => {}
                    }
                }
            }
            Msg::Clear => {
                let mut q = self.queue.lock().unwrap();
                q.files.retain(|e| {
                    matches!(e.stage, Stage::Waiting | Stage::Sending | Stage::Finishing)
                });
                q.note = None;
            }
            Msg::Forget(id) => {
                return Some(Box::new(move |c: &Client| {
                    c.admin_cancel_upload(&id)?;
                    Ok(Done::msg("Deleted the unfinished upload."))
                }));
            }
        }
        None
    }

    /// Ask for files and put them on the list, going to the folder shown.
    fn pick(&mut self, client: &Client) {
        let Some(root) = self.root_info().cloned() else {
            return;
        };
        let Some(files) = rfd::FileDialog::new()
            .set_title("Upload to the server")
            .add_filter("Videos, subtitles and images", &EXTENSIONS)
            .pick_files()
        else {
            return;
        };
        let rel = self.rel();
        let label = self.label(&root.id);
        let dest = if rel.is_empty() {
            label
        } else {
            format!("{label} / {rel}")
        };
        let mut q = self.queue.lock().unwrap();
        for local in files {
            let Some(name) = local.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let size = std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
            let key = q.next_key;
            q.next_key += 1;
            let stage = if size == 0 {
                Stage::Failed("the file is empty".into())
            } else {
                Stage::Waiting
            };
            q.files.push(Entry {
                key,
                local,
                name,
                size,
                root: root.id.clone(),
                path: rel.clone(),
                dest: dest.clone(),
                id: None,
                sent: 0,
                from: 0,
                began: None,
                stage,
                stop: false,
            });
        }
        if !q.running && q.files.iter().any(|e| e.stage == Stage::Waiting) {
            q.running = true;
            q.note = None;
            let (queue, c) = (self.queue.clone(), client.clone());
            std::thread::spawn(move || send_all(&queue, &c));
        }
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        if self.folders.missing {
            return not_set_up(Section::Upload, &cx.client.info.name);
        }
        let pw = w - 36.0; // inside a panel
        let mut col: Column<'a, LMsg> = column![];
        let q = self.queue.lock().unwrap();
        if !q.files.is_empty() || q.note.is_some() {
            col = col.push(queue_panel(&q, pw)).push(gap(16.0));
        }
        let ours: HashSet<String> = q.files.iter().filter_map(|e| e.id.clone()).collect();
        drop(q);
        col = col.push(self.folder_panel(cx, pw)).push(gap(16.0));
        let left: Vec<&UploadInfo> = self
            .unfinished
            .data
            .iter()
            .flatten()
            .filter(|u| !ours.contains(&u.id))
            .collect();
        if !left.is_empty() {
            col = col.push(unfinished_panel(left, |id| self.label(id), cx.busy));
        }
        col.into()
    }

    fn folder_panel<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        let mut col: Column<'a, LMsg> = column![caps("Where to", c(TEAL())), gap(8.0)];
        let Some(f) = self.folders.data.as_ref() else {
            if let Some(e) = &self.folders.error {
                return panel(col.push(error(format!("Couldn't load the folders: {e}"))));
            }
            return panel(col.push(icon::spinner(spin_secs(), style::MUTED(), 18.0)));
        };
        if f.roots.is_empty() {
            return panel(col.push(muted(
                "The server has no movie or series folder (MOVIES_DIR, SERIES_DIR).",
            )));
        }
        col = col.push(root_cards(&f.roots, self.root.as_deref(), w));
        let Some(root) = self.root_info() else {
            return panel(col);
        };
        let mut about = format!("On the server: {}", root.path);
        if root.total_bytes > 0 {
            about = format!(
                "{about}  ·  {} of {} free",
                size(root.free_bytes),
                size(root.total_bytes)
            );
        }
        col = col
            .push(gap(8.0))
            .push(mono(&about, 11.5).color(style::MUTED()));
        if !root.writable {
            col = col.push(gap(4.0)).push(
                text("The server can't write in this folder (its permissions).")
                    .size(13)
                    .color(c(WARN())),
            );
        }
        col = col.push(gap(14.0)).push(self.crumbs(root)).push(gap(10.0));

        // the actions, for the folder shown
        let here = f.root.as_ref() == self.root.as_ref() && f.path == self.rel();
        let can = here && root.writable && !cx.busy;
        let mut actions = row![ui::button_with(
            Btn::Primary,
            "Upload files here…",
            Some(Icon::Upload),
            can.then(|| send(Msg::Pick)),
        )]
        .spacing(10)
        .align_y(Vertical::Center);
        match &self.new_folder {
            None => {
                actions = actions.push(ui::button_with(
                    Btn::Small,
                    "New folder",
                    Some(Icon::Folder),
                    can.then(|| send(Msg::NewFolder(Some(String::new())))),
                ));
            }
            Some(name) => {
                let ok = can && !name.trim().is_empty();
                actions = actions
                    .push(
                        container(
                            text_input(new_folder_hint(root, self.path.len()), name)
                                .on_input(|s| send(Msg::NewFolder(Some(s))))
                                .on_submit_maybe(ok.then(|| send(Msg::MakeFolder)))
                                .size(14)
                                .padding([7, 10])
                                .style(super::super::profile::field_style),
                        )
                        .width(280),
                    )
                    .push(ui::button_with(
                        Btn::Small,
                        "Make it",
                        None,
                        ok.then(|| send(Msg::MakeFolder)),
                    ))
                    .push(ui::button_with(
                        Btn::Small,
                        "Cancel",
                        None,
                        Some(send(Msg::NewFolder(None))),
                    ));
            }
        }
        col = col.push(actions).push(gap(12.0));

        if let Some(e) = &self.folders.error {
            return panel(col.push(error(format!("Couldn't open the folder: {e}"))));
        }
        if !here {
            return panel(col.push(icon::spinner(spin_secs(), style::MUTED(), 16.0)));
        }
        if f.folders.len() > 12 {
            col = col
                .push(
                    container(
                        text_input("Filter folders", &self.filter)
                            .on_input(|s| send(Msg::Filter(s)))
                            .size(14)
                            .padding([7, 10])
                            .style(super::super::profile::field_style),
                    )
                    .max_width(360),
                )
                .push(gap(8.0));
        }
        let q = self.filter.trim().to_lowercase();
        let hits: Vec<_> = f
            .folders
            .iter()
            .filter(|d| q.is_empty() || d.name.to_lowercase().contains(&q))
            .collect();
        if f.folders.is_empty() && f.files.is_empty() {
            col = col.push(muted("This folder is empty."));
        }
        for d in hits.iter().take(SHOW) {
            col = col.push(folder_row(&d.name, w));
        }
        if hits.len() > SHOW {
            col = col.push(muted_owned(format!(
                "and {} more folders; filter to find them",
                hits.len() - SHOW
            )));
        }
        if !q.is_empty() && hits.is_empty() {
            col = col.push(muted("No folder here matches."));
        }
        for file in f.files.iter().take(SHOW) {
            col = col.push(
                container(
                    row![
                        text(file.name.clone())
                            .size(13.5)
                            .color(style::MUTED())
                            .width(Length::Fill),
                        mono(&size(file.size), 11.5).color(style::MUTED()),
                    ]
                    .spacing(12)
                    .align_y(Vertical::Center),
                )
                .padding(Padding::default().left(26.0).top(3.0).bottom(3.0))
                .width(w),
            );
        }
        panel(col)
    }

    /// "Disk 1 · Movies / Heat (1995) / Subs", each part a way back up.
    fn crumbs<'a>(&'a self, root: &UploadRoot) -> Element<'a, LMsg> {
        let mut r = row![crumb(self.label(&root.id), !self.path.is_empty(), 0)]
            .spacing(6)
            .align_y(Vertical::Center);
        for (i, part) in self.path.iter().enumerate() {
            let last = i + 1 == self.path.len();
            r = r.push(text("/").size(14).color(style::MUTED())).push(crumb(
                part.clone(),
                !last,
                i + 1,
            ));
        }
        r.wrap().into()
    }
}

fn send(m: Msg) -> LMsg {
    LMsg::Admin(AMsg::Upload(m))
}

fn muted<'a>(t: &'static str) -> Element<'a, LMsg> {
    text(t).size(14).color(style::MUTED()).into()
}

fn muted_owned<'a>(t: String) -> Element<'a, LMsg> {
    text(t).size(13).color(style::MUTED()).into()
}

fn error<'a>(t: String) -> Element<'a, LMsg> {
    text(t).size(14).color(style::ERROR()).into()
}

/// What to call a new folder here, as the library expects it.
fn new_folder_hint(root: &UploadRoot, depth: usize) -> &'static str {
    match (root.kind.as_str(), depth) {
        ("series", 0) => "Show name, e.g. Dark",
        ("series", 1) => "Season 01",
        (_, 0) => "Title.Year, e.g. Heat.1995",
        _ => "Folder name",
    }
}

/// 2090000000 -> "2.09 GB", 4.2e12 -> "4.20 TB".
fn size(n: u64) -> String {
    if n as f64 >= 1e12 {
        format!("{:.2} TB", n as f64 / 1e12)
    } else {
        crate::ui::browse::details::bytes(n)
    }
}

/// "12.4 MB/s".
fn speed(bytes: u64, secs: f64) -> Option<String> {
    (secs >= 1.0 && bytes > 0).then(|| format!("{:.1} MB/s", bytes as f64 / secs / 1e6))
}

/// "3 min left".
fn left(bytes: u64, rate: f64) -> String {
    let s = (bytes as f64 / rate.max(1.0)) as u64;
    match s {
        s if s < 60 => format!("{s} s left"),
        s if s < 3600 => format!("{} min left", s / 60 + 1),
        s => format!("{} h {} min left", s / 3600, s % 3600 / 60),
    }
}

/// The roots side by side, each with its free space: one is shown.
fn root_cards<'a>(roots: &[UploadRoot], on: Option<&str>, w: f32) -> Element<'a, LMsg> {
    const GAP: f32 = 10.0;
    let per = roots.len().clamp(1, 4);
    // a little short of `w`: rounding and borders mustn't wrap the last one
    let cw = ((w - 12.0 - GAP * (per as f32 - 1.0)) / per as f32).floor();
    let mut r = row![].spacing(GAP);
    for root in roots {
        let sel = on == Some(root.id.as_str());
        let used = if root.total_bytes > 0 {
            1.0 - root.free_bytes as f32 / root.total_bytes as f32
        } else {
            0.0
        };
        let mut body = column![
            text(root.label()).size(14.5),
            text(format!("{} free", size(root.free_bytes)))
                .size(12.5)
                .color(style::MUTED()),
        ]
        .spacing(4);
        if root.total_bytes > 0 {
            body = body.push(space().height(4)).push(fill_bar(used, cw - 28.0));
        }
        r = r.push(
            button(body)
                .width(cw)
                .padding([10, 14])
                .style(move |_, s| {
                    let hot = matches!(s, button::Status::Hovered);
                    button::Style {
                        background: Some(Background::Color(c(if sel || hot {
                            SURFACE_HI()
                        } else {
                            CARD()
                        }))),
                        text_color: style::TEXT(),
                        border: Border {
                            color: if sel {
                                ui::accent()
                            } else if hot {
                                c(LINE_HI())
                            } else {
                                c(LINE())
                            },
                            width: if sel { 1.5 } else { 1.0 },
                            radius: 8.0.into(),
                        },
                        ..button::Style::default()
                    }
                })
                .on_press(send(Msg::Root(root.id.clone()))),
        );
    }
    r.wrap().vertical_spacing(GAP).into()
}

/// How full a disk is: a thin bar, red past 90 %.
fn fill_bar<'a>(f: f32, w: f32) -> Element<'a, LMsg> {
    let tone = if f > 0.9 { c(ERROR()) } else { ui::accent() };
    container(
        container(space())
            .width((w * f.clamp(0.0, 1.0)).max(2.0))
            .height(4)
            .style(move |_| container::Style {
                background: Some(Background::Color(tone)),
                border: iced::border::rounded(2),
                ..container::Style::default()
            }),
    )
    .width(w)
    .height(4)
    .style(|_| container::Style {
        background: Some(Background::Color(c(LINE()))),
        border: iced::border::rounded(2),
        ..container::Style::default()
    })
    .into()
}

fn crumb<'a>(t: String, active: bool, depth: usize) -> Element<'a, LMsg> {
    let b = button(text(t).size(14))
        .padding(0)
        .style(move |_, s| button::Style {
            text_color: if !active {
                style::TEXT()
            } else if matches!(s, button::Status::Hovered) {
                ui::accent()
            } else {
                style::MUTED()
            },
            ..button::Style::default()
        });
    if active {
        b.on_press(send(Msg::Up(depth))).into()
    } else {
        b.into()
    }
}

/// A folder to go into.
fn folder_row<'a>(name: &str, w: f32) -> Element<'a, LMsg> {
    button(
        row![
            icon::icon(Icon::Folder, style::MUTED(), 15.0),
            text(name.to_owned()).size(14),
        ]
        .spacing(10)
        .align_y(Vertical::Center),
    )
    .width(w)
    .padding([5, 4])
    .style(|_, s| button::Style {
        text_color: if matches!(s, button::Status::Hovered) {
            ui::accent()
        } else {
            style::TEXT()
        },
        ..button::Style::default()
    })
    .on_press(send(Msg::Open(name.to_owned())))
    .into()
}

/// The files on their way, and how far each got.
fn queue_panel<'a>(q: &Queue, w: f32) -> Element<'a, LMsg> {
    let going = q
        .files
        .iter()
        .filter(|e| matches!(e.stage, Stage::Waiting | Stage::Sending | Stage::Finishing))
        .count();
    let mut head = row![caps("Uploads", c(TEAL())), space::horizontal()]
        .spacing(10)
        .align_y(Vertical::Center);
    if q.files.len() > going {
        head = head.push(ui::button_with(
            Btn::Small,
            "Clear finished",
            None,
            Some(send(Msg::Clear)),
        ));
    }
    let mut col: Column<'a, LMsg> = column![head, gap(8.0)];
    if going > 0 {
        col = col
            .push(
                text(
                    "Keep Valo open until these are done. If it closes, pick the same \
                     file into the same folder again and it carries on where it stopped.",
                )
                .size(12.5)
                .color(style::MUTED()),
            )
            .push(gap(10.0));
    }
    for e in &q.files {
        col = col.push(entry_row(e, w)).push(gap(10.0));
    }
    if q.running && going == 0 {
        col = col.push(
            row![
                icon::spinner(spin_secs(), style::MUTED(), 14.0),
                text("Adding the new files to the library…").size(13),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        );
    }
    if let Some((msg, ok)) = &q.note {
        col = col.push(
            text(msg.clone())
                .size(13)
                .color(if *ok { c(OK()) } else { c(ERROR()) }),
        );
    }
    panel(col)
}

fn entry_row<'a>(e: &Entry, w: f32) -> Element<'a, LMsg> {
    let f = if e.size > 0 {
        e.sent as f32 / e.size as f32
    } else {
        0.0
    };
    let mut value = format!("{} of {}", size(e.sent), size(e.size));
    let (status, tone): (String, _) = match &e.stage {
        Stage::Waiting => ("Waiting".into(), style::MUTED()),
        Stage::Sending => {
            let secs = e.began.map_or(0.0, |t| t.elapsed().as_secs_f64());
            let done = e.sent.saturating_sub(e.from);
            if let Some(s) = speed(done, secs) {
                value = format!(
                    "{value}  ·  {s}  ·  {}",
                    left(e.size - e.sent, done as f64 / secs)
                );
            }
            if e.stop {
                ("Stopping…".into(), style::MUTED())
            } else if e.from > 0 && done == 0 {
                (format!("Carrying on from {}", size(e.from)), style::MUTED())
            } else {
                ("Uploading".into(), style::TEXT())
            }
        }
        Stage::Finishing => ("Moving it into place…".into(), style::MUTED()),
        Stage::Done => ("Done".into(), c(OK())),
        Stage::Failed(why) => (format!("Failed: {why}"), c(ERROR())),
        Stage::Stopped => ("Stopped".into(), style::MUTED()),
    };
    let mut top = row![
        column![
            text(e.name.clone()).size(14),
            mono(&format!("→ {}", e.dest), 11.0).color(style::MUTED()),
        ]
        .spacing(2)
        .width(Length::Fill),
        text(status).size(12.5).color(tone),
    ]
    .spacing(12)
    .align_y(Vertical::Center);
    let stoppable = matches!(e.stage, Stage::Waiting | Stage::Sending) && !e.stop;
    if stoppable {
        top = top.push(ui::button_with(
            Btn::Small,
            "Stop",
            None,
            Some(send(Msg::Cancel(e.key))),
        ));
    }
    let mut col = column![top].spacing(6).width(w);
    if matches!(e.stage, Stage::Sending | Stage::Finishing)
        || (e.sent > 0 && e.stage != Stage::Done)
    {
        col = col.push(ui::bar("", value, f, w));
    }
    col.into()
}

/// Uploads the server still has half of, from earlier.
fn unfinished_panel<'a>(
    list: Vec<&'a UploadInfo>,
    label: impl Fn(&str) -> String,
    busy: bool,
) -> Element<'a, LMsg> {
    let mut col: Column<'a, LMsg> = column![
        caps(
            &format!("Unfinished on the server · {}", list.len()),
            c(WARN())
        ),
        gap(6.0),
        text(
            "To carry on with one, pick the same file into the same folder. \
             Delete removes what arrived.",
        )
        .size(12.5)
        .color(style::MUTED()),
        gap(10.0),
    ];
    for u in list {
        let root = label(&u.root);
        let dest = if u.path.is_empty() {
            root
        } else {
            format!("{root} / {}", u.path)
        };
        let mut when = String::new();
        if let Some(t) = api::iso_unix(&u.started_at) {
            when = format!(" · started {}", crate::ui::browse::people::ago(t));
        }
        col = col.push(
            row![
                column![
                    text(u.filename.clone()).size(14),
                    mono(
                        &format!(
                            "→ {dest} · {} of {} · by {}{when}",
                            size(u.offset),
                            size(u.size),
                            u.by
                        ),
                        11.0
                    )
                    .color(style::MUTED()),
                ]
                .spacing(2)
                .width(Length::Fill),
                ui::danger("Delete", (!busy).then(|| send(Msg::Forget(u.id.clone())))),
            ]
            .spacing(12)
            .align_y(Vertical::Center),
        );
        col = col.push(gap(8.0));
    }
    panel(col)
}

// ── The thread sending the files ─────────────────────────────────────────

/// Send every waiting file, one at a time, then have the server rescan.
/// Files added meanwhile are sent too.
fn send_all(queue: &Arc<Mutex<Queue>>, c: &Client) {
    loop {
        let mut sent = 0;
        loop {
            let key = {
                let mut q = queue.lock().unwrap();
                let Some(e) = q.files.iter_mut().find(|e| e.stage == Stage::Waiting) else {
                    break;
                };
                e.stage = Stage::Sending;
                e.key
            };
            let outcome = send_one(queue, c, key);
            let mut q = queue.lock().unwrap();
            let Some(e) = q.files.iter_mut().find(|e| e.key == key) else {
                continue;
            };
            e.stage = match outcome {
                Ok(true) => {
                    sent += 1;
                    e.sent = e.size;
                    Stage::Done
                }
                Ok(false) => Stage::Stopped,
                Err(err) if err.is::<api::Unauthorized>() => {
                    Stage::Failed("signed out; sign in again and pick it again".into())
                }
                Err(err) => Stage::Failed(format!("{err:#}")),
            };
        }
        let note = (sent > 0).then(|| match c.admin_rescan() {
            Ok(_) => (
                format!(
                    "Uploaded {sent} file{}. The server is adding them to the library; \
                     new titles show up in a minute or two.",
                    if sent == 1 { "" } else { "s" }
                ),
                true,
            ),
            Err(e) => (
                format!("Uploaded, but the rescan failed ({e:#}). Run Rescan on the dashboard."),
                false,
            ),
        });
        let mut q = queue.lock().unwrap();
        if note.is_some() {
            q.note = note;
        }
        // picked while the rescan ran: go on
        if q.files.iter().any(|e| e.stage == Stage::Waiting) {
            continue;
        }
        q.running = false;
        q.batches += 1;
        return;
    }
}

/// Send one file; false when it was stopped.
fn send_one(queue: &Arc<Mutex<Queue>>, c: &Client, key: u64) -> Result<bool> {
    let (local, root, path, name, size) = {
        let q = queue.lock().unwrap();
        let Some(e) = q.files.iter().find(|e| e.key == key) else {
            return Ok(false);
        };
        (
            e.local.clone(),
            e.root.clone(),
            e.path.clone(),
            e.name.clone(),
            e.size,
        )
    };
    let info = c.admin_start_upload(&root, &path, &name, size)?;
    let mut off = info.offset;
    update(queue, key, |e| {
        e.id = Some(info.id.clone());
        e.sent = off;
        e.from = off;
        e.began = Some(Instant::now());
    });
    let chunk = match info.chunk_size {
        0 => CHUNK,
        n => n.clamp(CHUNK_MIN, CHUNK_MAX),
    };
    let mut file = std::fs::File::open(&local)?;
    if file.metadata()?.len() != size {
        bail!("the file changed since it was picked");
    }
    let mut buf = vec![0u8; chunk as usize];
    let mut tries = 0;
    while off < size {
        if stopping(queue, key) {
            let _ = c.admin_cancel_upload(&info.id);
            return Ok(false);
        }
        let n = chunk.min(size - off) as usize;
        file.seek(SeekFrom::Start(off))?;
        file.read_exact(&mut buf[..n])?;
        match c.admin_upload_chunk(&info.id, off, &buf[..n]) {
            Ok(Chunk::Moved(o)) if o == off || o > size => {
                bail!("the server and the app disagree on how much arrived")
            }
            Ok(Chunk::Taken(o) | Chunk::Moved(o)) => {
                off = o;
                tries = 0;
                update(queue, key, |e| e.sent = off);
            }
            Err(e) if e.is::<api::Unauthorized>() => return Err(e),
            Err(e) => {
                tries += 1;
                if tries >= TRIES {
                    return Err(e);
                }
                // the connection dropped: wait, then try again (a chunk
                // that did arrive comes back as Moved)
                std::thread::sleep(Duration::from_secs(1 << tries.min(5)));
            }
        }
    }
    if stopping(queue, key) {
        let _ = c.admin_cancel_upload(&info.id);
        return Ok(false);
    }
    update(queue, key, |e| e.stage = Stage::Finishing);
    c.admin_finish_upload(&info.id)?;
    Ok(true)
}

fn update(queue: &Arc<Mutex<Queue>>, key: u64, f: impl FnOnce(&mut Entry)) {
    let mut q = queue.lock().unwrap();
    if let Some(e) = q.files.iter_mut().find(|e| e.key == key) {
        f(e);
    }
}

fn stopping(queue: &Arc<Mutex<Queue>>, key: u64) -> bool {
    let q = queue.lock().unwrap();
    q.files.iter().find(|e| e.key == key).is_none_or(|e| e.stop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(size(4_200_000_000_000), "4.20 TB");
        assert_eq!(size(2_090_000_000), "2.09 GB");
        assert_eq!(speed(25_000_000, 2.0).as_deref(), Some("12.5 MB/s"));
        assert_eq!(speed(25_000_000, 0.5), None);
        assert_eq!(left(30_000_000, 1_000_000.0), "30 s left");
        assert_eq!(left(600_000_000, 1_000_000.0), "11 min left");
        assert_eq!(left(7_200_000_000, 1_000_000.0), "2 h 0 min left");
    }
}
