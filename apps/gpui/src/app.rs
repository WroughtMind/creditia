//! Production desktop view. Fixture construction uses the same render tree and
//! event handlers while keeping project data and filesystem services isolated.
use crate::{renderer,native_preview,ui_model::*};
use crate::{Appearance,Language,Preferences,Project,export_filename,export_with_preferences,progress_text};
use creditia_desktop_service as service;
use gpui_kit::component::{ActiveTheme,Disableable,Theme,ThemeMode};
use gpui_kit::component::button::{Button,ButtonVariants};
use gpui_kit::component::input::{Input,InputState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use gpui_kit::TestSupportExt as _;
use serde::Deserialize;
use serde_json::{Value,json};
use std::{collections::BTreeMap,path::Path,sync::Arc,time::{Duration,Instant}};

actions!(creditia,[Quit,OpenProject,Refresh,Analyze,CancelAnalysis,CancelCurrent,CopyArchive,RevealProject,ToggleLanguage,CycleAppearance,ToggleMotion,ExportArtwork,CancelExport,SwitchProject,Settings,ProjectSettings,Wall,Roam,PreviousStyle,NextStyle,ZoomIn,ZoomOut,ResetZoom,Play,Replay,PreviousPiece,NextPiece]);
#[derive(Clone,Copy)] enum ExportStage{ChoosePath,Rendering,Writing}
enum JobEvent{Progress(Value),Finished(Result<Value,service::Fail>)}
#[derive(Clone,Copy,PartialEq,Eq)]enum Panel{Projects,Styles,Pieces,Settings,Project,Details}
#[derive(Clone)]struct Tile{spec:TileSpec,art:Option<Artwork>,image:Option<Arc<Image>>,pixels:u32,error:Option<String>,loading:bool}
struct Playback{frames:Vec<(Artwork,Arc<Image>)>,times:Vec<f64>,timeline:Timeline,started:Instant,elapsed:Duration,playing:bool,index:usize}
struct DragState{tile:Option<usize>,start:(f32,f32),last:(f32,f32),moved:bool}
#[derive(Clone,Deserialize)]
pub struct Fixture{
    pub raw:Value,pub catalog:Value,#[serde(default)]pub artworks:Vec<Value>,
    #[serde(default)]pub language:Language,#[serde(default)]pub appearance:Appearance,
}

pub struct Creditia{
    focus:FocusHandle,preferences:Preferences,projects:Vec<Project>,selected:Option<String>,raw:Option<Value>,
    loading:bool,error:Option<String>,notice:Option<String>,progress:Option<Value>,job_busy:bool,cancelling:bool,
    analysis_cancellation:Option<service::AnalysisCancellation>,list_generation:u64,archive_generation:u64,
    list_task:Option<Task<()>>,archive_task:Option<Task<()>>,dialog_task:Option<Task<()>>,job_task:Option<Task<()>>,job_worker:Option<Task<()>>,
    catalog:Option<Value>,style_index:usize,tiles:Vec<Tile>,dimensions:BTreeMap<String,(f32,f32)>,rects:Vec<Rect>,wall_height:f32,
    viewport:Viewport,viewport_size:(f32,f32),selected_tile:Option<usize>,focused:bool,hover:Option<Hit>,pointer:(f32,f32),drag:Option<DragState>,
    preview_generation:u64,preview_task:Option<Task<()>>,preview_cancellation:Option<renderer::Cancellation>,preview_busy:bool,
    playback:Option<Playback>,animation_task:Option<Task<()>>,animation_cancellation:Option<renderer::Cancellation>,animation_busy:bool,
    panel:Option<Panel>,details:Option<Value>,collect:Value,operation_task:Option<Task<()>>,operation_busy:bool,
    sign_input:Entity<InputState>,search_input:Entity<InputState>,all_archives:Vec<Value>,
    export_format:String,export_size:String,transparent:bool,font_mode:String,export_label:String,export_stage:Option<ExportStage>,
    export_cancel_requested:bool,export_task:Option<Task<()>>,export_cancellation:Option<renderer::Cancellation>,export_open:bool,
    fixture:bool,background_task:Option<Task<()>>,background:Option<service::BackgroundService>,
}
impl Creditia{
    fn base(preferences:Preferences,fixture:bool,window:&mut Window,cx:&mut Context<Self>)->Self{
        bind_keys(cx);apply_appearance(&preferences.appearance,window,cx);install_menus(&preferences.language,cx);cx.set_reduce_motion(preferences.reduced_motion);
        let focus=cx.focus_handle().tab_stop(true);focus.focus(window,cx);
        let sign_input=cx.new(|cx|InputState::new(window,cx).placeholder("署名 / Signature"));
        let search_input=cx.new(|cx|InputState::new(window,cx).placeholder("搜索项目 / Search projects"));
        Self{focus,selected:preferences.last_project.clone(),export_format:preferences.export_format.clone(),export_size:preferences.export_size.clone(),preferences,
            projects:vec![],raw:None,loading:false,error:None,notice:None,progress:None,job_busy:false,cancelling:false,analysis_cancellation:None,
            list_generation:0,archive_generation:0,list_task:None,archive_task:None,dialog_task:None,job_task:None,job_worker:None,
            catalog:None,style_index:0,tiles:vec![],dimensions:BTreeMap::new(),rects:vec![],wall_height:0.,viewport:Viewport::default(),viewport_size:(1000.,700.),selected_tile:None,focused:false,hover:None,pointer:(0.,0.),drag:None,
            preview_generation:0,preview_task:None,preview_cancellation:None,preview_busy:false,playback:None,animation_task:None,animation_cancellation:None,animation_busy:false,
            panel:None,details:None,collect:Value::Null,operation_task:None,operation_busy:false,sign_input,search_input,all_archives:vec![],
            transparent:false,font_mode:"outline".into(),export_label:String::new(),export_stage:None,export_cancel_requested:false,export_task:None,export_cancellation:None,export_open:false,
            fixture,background_task:None,background:None}
    }
    pub fn new(window:&mut Window,cx:&mut Context<Self>)->Self{
        let mut view=Self::base(Preferences::read(&service::ui_preferences()),false,window,cx);view.refresh(cx);view.start_background(cx);view
    }
    pub fn fixture(fixture:Fixture,window:&mut Window,cx:&mut Context<Self>)->Self{
        let mut preferences=Preferences{language:fixture.language,appearance:fixture.appearance,..Default::default()};
        if let Some(style)=fixture.artworks.first().and_then(|a|a["style"].as_str()){preferences.style=style.into();}
        let mut view=Self::base(preferences,true,window,cx);view.selected=Some("fixture".into());view.preferences.last_project=view.selected.clone();
        view.projects.push(Project{id:"fixture".into(),name:fixture.raw["project"]["name"].as_str().unwrap_or("Creditia").into(),path:"".into(),exists:true,has_credits:true,evidence:0});
        view.raw=Some(fixture.raw);view.catalog=Some(fixture.catalog);view.resolve_style();view.rebuild_tiles(cx);
        for value in fixture.artworks{let piece=value["piece"].as_str().unwrap_or("");let variant=value["variant"].as_str();if let Ok(art)=serde_json::from_value::<Artwork>(value["art"].clone()){
            for tile in &mut view.tiles{if tile.spec.piece==piece&&tile.spec.variant.as_deref()==variant{tile.image=Some(Arc::new(Image::from_bytes(ImageFormat::Svg,art.svg.as_bytes().to_vec())));tile.pixels=4096;tile.art=Some(art.clone());view.dimensions.insert(tile.spec.key.clone(),(art.w,art.h));}}
        }}view
    }
    fn text(&self,zh:&'static str,en:&'static str)->&'static str{self.preferences.language.text(zh,en)}
    fn persist(&mut self){if self.fixture{return;}if let Err(e)=service::patch_ui_preferences(self.preferences.shared_patch()){self.error=Some(e.to_string());}}
    fn project_preferences(&self)->crate::ProjectPreferences{self.preferences.project(self.selected.as_deref())}
    fn group(&self)->String{self.project_preferences().group}
    fn current(&self)->Option<&Project>{self.projects.iter().find(|p|Some(&p.id)==self.selected.as_ref())}
    fn style(&self)->Option<&Value>{self.catalog.as_ref()?.get("styles")?.as_array()?.get(self.style_index)}
    fn piece(&self,id:&str)->Option<&Value>{self.style()?.get("pieces")?.as_array()?.iter().find(|p|p["id"]==id)}
    fn current_tile(&self)->Option<&Tile>{self.selected_tile.and_then(|i|self.tiles.get(i))}
    fn resolve_style(&mut self){self.style_index=self.catalog.as_ref().and_then(|c|c["styles"].as_array()).and_then(|s|s.iter().position(|s|s["id"]==self.preferences.style)).unwrap_or(0);}
    fn stop_playback(&mut self,cx:&mut Context<Self>){if let Some(c)=self.animation_cancellation.take(){c.cancel();}self.animation_task=None;self.animation_busy=false;if let Some(play)=self.playback.take(){for(_,image)in play.frames{ImageSource::Image(image).remove_asset(cx);}}}
    fn clear_preview(&mut self,cx:&mut Context<Self>){if let Some(c)=self.preview_cancellation.take(){c.cancel();}self.preview_generation=self.preview_generation.wrapping_add(1);self.preview_task=None;self.preview_busy=false;self.stop_playback(cx);for tile in &mut self.tiles{if let Some(image)=tile.image.take(){ImageSource::Image(image).remove_asset(cx);}}self.tiles.clear();self.rects.clear();self.hover=None;}
    fn rebuild_tiles(&mut self,cx:&mut Context<Self>){self.clear_preview(cx);let prefs=self.project_preferences();if let Some(style)=self.style(){self.tiles=wall_tiles(style,&prefs.variants,&prefs.order).into_iter().map(|spec|Tile{spec,art:None,image:None,pixels:0,error:None,loading:false}).collect();}self.selected_tile=(!self.tiles.is_empty()).then_some(0);self.viewport.reset();self.focused=false;cx.notify();}
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.list_generation = self.list_generation.wrapping_add(1);
        let generation = self.list_generation;
        self.loading = true;
        self.error = None;
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async { service::projects() }).await;
            let _ = this.update(cx, |view, cx| {
                if view.list_generation != generation { return; }
                view.loading = false;
                match result {
                    Ok(value) => match serde_json::from_value::<Vec<Project>>(value) {
                        Ok(projects) => {
                            view.projects = projects;
                            let id = view.selected.clone()
                                .filter(|id| view.projects.iter().any(|p| &p.id == id))
                                .or_else(|| view.projects.first().map(|p| p.id.clone()));
                            if let Some(id) = id { view.select(id, cx); }
                            else {
                                view.archive_generation = view.archive_generation.wrapping_add(1);
                                view.archive_task = None;
                                view.clear_preview(cx);
                                view.selected = None;
                                view.raw = None;
                            }
                        }
                        Err(error) => view.error = Some(error.to_string()),
                    },
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        self.clear_preview(cx);
        self.archive_generation = self.archive_generation.wrapping_add(1);
        let generation = self.archive_generation;
        self.selected = Some(id.clone());
        self.all_archives.clear();
        self.focused = false; self.details = None;
        if let Some(background) = &self.background { background.watch_project(Some(id.clone())); }
        self.preferences.last_project = Some(id.clone());
        self.error = None;
        self.persist();
        self.raw = None;
        self.loading = true;
        self.archive_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { service::credits(id) }).await;
            let _ = this.update(cx, |view, cx| {
                // A slower previous project must never replace the current archive.
                if view.archive_generation != generation { return; }
                view.loading = false;
                match result {
                    Ok(raw) => { view.raw = Some(raw); view.load_catalog(cx); },
                    Err(error) if error.code == "no_credits" => {},
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn open_project(&mut self, _: &OpenProject, _: &mut Window, cx: &mut Context<Self>) {
        if self.job_busy { return; }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false, directories: true, multiple: false,
            prompt: Some(self.text("选择 Git 项目", "Choose a Git project").into()),
        });
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let _ = this.update(cx, |view, cx| {
                match choice {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            view.start_job(Some(path.to_string_lossy().into_owned()), cx);
                        }
                    }
                    Ok(Ok(None)) => {},
                    Ok(Err(error)) => view.error = Some(error.to_string()),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }

    fn start_job(&mut self, add_path: Option<String>, cx: &mut Context<Self>) {
        if self.job_busy || (add_path.is_none() && self.selected.is_none()) { return; }
        self.job_busy = true;
        self.cancelling = false;
        self.progress = None;
        self.error = None;
        self.notice = None;
        let selected = self.selected.clone();
        let selection_at_start = selected.clone();
        let lang = self.preferences.language.code().to_string();
        let adding = add_path.is_some();
        let cancellation = service::AnalysisCancellation::default();
        self.analysis_cancellation = Some(cancellation.clone());
        // Progress is bounded and disposable; completion is always delivered.
        let (sender, receiver) = async_channel::bounded(32);
        let progress_sender = sender.clone();
        let sink: service::ProgressSink = Arc::new(move |value| {
            let _ = progress_sender.try_send(JobEvent::Progress(value));
        });
        self.job_worker = Some(cx.background_spawn(async move {
            let result = match add_path {
                Some(path) => service::add_project_with_cancel(path, Some(lang), Some(sink), cancellation),
                None => service::analyze_with_cancel(selected.expect("selected project"), Some(lang), Some(sink), cancellation),
            };
            let _ = sender.send(JobEvent::Finished(result)).await;
        }));
        self.job_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                let finished = matches!(&event, JobEvent::Finished(_));
                if this.update(cx, |view, cx| {
                    match event {
                        JobEvent::Progress(value) => view.progress = Some(value),
                        JobEvent::Finished(result) => {
                            view.job_busy = false;
                            view.analysis_cancellation = None;
                            view.progress = None;
                            match result {
                                Ok(value) => {
                                    if adding && view.selected == selection_at_start {
                                        view.selected = value["id"].as_str().map(str::to_string);
                                    }
                                    view.notice = Some(view.text("分析完成", "Analysis complete").into());
                                    view.refresh(cx);
                                }
                                Err(error) if view.cancelling || error.code == "cancelled" => {
                                    view.notice = Some(view.text("已取消分析", "Analysis cancelled").into());
                                }
                                Err(error) => view.error = Some(error.to_string()),
                            }
                            view.cancelling = false;
                        }
                    }
                    cx.notify();
                }).is_err() { break; }
                if finished { break; }
            }
        }));
        cx.notify();
    }

    fn analyze(&mut self, _: &Analyze, _: &mut Window, cx: &mut Context<Self>) {
        self.start_job(None, cx);
    }

    fn cancel(&mut self, _: &CancelAnalysis, _: &mut Window, cx: &mut Context<Self>) {
        if self.job_busy {
            if let Some(cancellation) = &self.analysis_cancellation { cancellation.cancel(); }
            self.cancelling = true;
            cx.notify();
        }
    }

    fn copy_archive(&mut self, _: &CopyArchive, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(raw) = &self.raw {
            match serde_json::to_string_pretty(raw) {
                Ok(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.notice = Some(self.text("档案已复制", "Archive copied").into());
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            cx.notify();
        }
    }

    fn reveal_project(&mut self, _: &RevealProject, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.current() { cx.reveal_path(Path::new(&project.path)); }
    }

    fn load_catalog(&mut self,cx:&mut Context<Self>){
        if self.catalog.is_some(){self.resolve_style();self.rebuild_tiles(cx);return;}
        let cancellation=renderer::Cancellation::default();self.preview_cancellation=Some(cancellation.clone());let generation=self.preview_generation;self.preview_busy=true;
        self.preview_task=Some(cx.spawn(async move|this,cx|{let result=cx.background_spawn(async move{renderer::request(json!({"version":1,"id":"catalog","op":"catalog"}),cancellation)}).await;let _=this.update(cx,|view,cx|{if view.preview_generation!=generation{return;}view.preview_busy=false;match result{Ok(catalog)=>{view.catalog=Some(catalog);view.resolve_style();view.rebuild_tiles(cx);},Err(error)=>view.error=Some(error)}cx.notify();});}));
    }
    fn artwork_request(&self,spec:&TileSpec,op:&str)->Option<Value>{
        let mut request=json!({"version":1,"id":format!("{}:{}",self.preview_generation,spec.key),"op":op,"raw":self.raw.as_ref()?,"style":self.style()?["id"],"piece":spec.piece,"lang":self.preferences.language.code(),"group":self.group(),"named":self.preferences.named,"flatten":true});
        if let Some(variant)=&spec.variant{request["variant"]=json!(variant);}
        if !self.all_archives.is_empty(){request["projects"]=json!(self.all_archives);}
        Some(request)
    }
    fn compute_layout(&mut self,width:f32,height:f32){
        self.viewport_size=(width,height);let world_width=if self.preferences.view=="roam"{1440.}else{width};
        let specs=self.tiles.iter().map(|t|t.spec.clone()).collect::<Vec<_>>();let(rects,total)=masonry(&specs,&self.dimensions,world_width);self.rects=rects;self.wall_height=total;
        if self.preferences.view=="wall"&&!self.focused{self.viewport.x=0.;self.viewport.zoom=1.;self.viewport.y=self.viewport.y.clamp(-(total-height).max(0.),0.);}
    }
    fn screen_rect(&self,index:usize)->Rect{
        if self.focused&&self.selected_tile==Some(index){let art=self.tiles.get(index).and_then(|t|t.art.as_ref());let(w,h)=art.map(|a|(a.w,a.h)).unwrap_or((640.,480.));let scale=((self.viewport_size.0-100.)/w).min((self.viewport_size.1-100.)/h).max(0.01)*self.viewport.zoom;Rect{x:(self.viewport_size.0-w*scale)/2.+self.viewport.x,y:(self.viewport_size.1-h*scale)/2.+self.viewport.y,w:w*scale,h:h*scale}}
        else{self.rects.get(index).copied().unwrap_or_default().transformed(self.viewport)}
    }
    fn schedule_visible(&mut self,dpi:f32,cx:&mut Context<Self>){
        if self.fixture||self.preview_busy||self.raw.is_none()||self.catalog.is_none(){return;}
        let visible=Rect{x:-150.,y:-220.,w:self.viewport_size.0+300.,h:self.viewport_size.1+440.};
        let next=self.tiles.iter().enumerate().filter(|(i,t)|(!self.focused||self.selected_tile==Some(*i))&&!t.loading&&t.error.is_none()).find_map(|(index,tile)|{let r=self.screen_rect(index);if !r.intersects(visible){return None;}let pixels=texture_width(r.w,dpi);(tile.image.is_none()||tile.pixels<pixels).then_some((index,pixels))});
        let Some((index,pixels))=next else{return;};let Some(request)=self.artwork_request(&self.tiles[index].spec,"render")else{return;};let existing=self.tiles[index].art.clone();let key=self.tiles[index].spec.key.clone();let generation=self.preview_generation;
        self.tiles[index].loading=true;self.preview_busy=true;let cancellation=renderer::Cancellation::default();self.preview_cancellation=Some(cancellation.clone());
        self.preview_task=Some(cx.spawn(async move|this,cx|{let result:Result<(Artwork,Arc<native_preview::RasterImage>),String>=cx.background_spawn(async move{let art=match existing{Some(art)=>art,None=>{let result=renderer::request(request,cancellation.clone())?;if result["art"].is_null(){return Err("__empty__".into());}serde_json::from_value::<Artwork>(result["art"].clone()).map_err(|e|e.to_string())?}};let raster=native_preview::rasterize(&art.svg,pixels,&cancellation)?;Ok((art,raster))}).await;
            let _=this.update(cx,|view,cx|{if generation!=view.preview_generation{return;}view.preview_busy=false;view.preview_cancellation=None;if let Some(tile)=view.tiles.get_mut(index).filter(|t|t.spec.key==key){tile.loading=false;match result{Ok((art,raster))=>{if let Some(image)=tile.image.take(){ImageSource::Image(image).remove_asset(cx);}tile.image=Some(Arc::new(Image::from_bytes(ImageFormat::Png,raster.png.clone())));tile.pixels=raster.width;view.dimensions.insert(key,(art.w,art.h));tile.art=Some(art);},Err(error)=>tile.error=Some(error)}}view.evict_distant(cx);cx.notify();});}));
    }
    fn evict_distant(&mut self,cx:&mut Context<Self>){
        let mut bytes=self.tiles.iter().filter_map(|t|t.art.as_ref().map(|a|(t.pixels as f64).powi(2)*(a.h/a.w)as f64*4.)).sum::<f64>();if bytes<128.*1024.*1024.{return;}
        let near=Rect{x:-self.viewport_size.0,y:-self.viewport_size.1,w:self.viewport_size.0*3.,h:self.viewport_size.1*3.};
        for index in (0..self.tiles.len()).rev(){if bytes<96.*1024.*1024.{break;}if self.selected_tile==Some(index)||self.screen_rect(index).intersects(near){continue;}let tile=&mut self.tiles[index];if let Some(image)=tile.image.take(){ImageSource::Image(image).remove_asset(cx);if let Some(art)=&tile.art{bytes-=(tile.pixels as f64).powi(2)*(art.h/art.w)as f64*4.;}tile.pixels=0;}}
    }
    fn choose_style(&mut self,index:usize,cx:&mut Context<Self>){if let Some(style)=self.catalog.as_ref().and_then(|c|c["styles"].as_array()).and_then(|s|s.get(index)){self.preferences.style=style["id"].as_str().unwrap_or("").into();self.style_index=index;self.persist();self.panel=None;self.rebuild_tiles(cx);}}
    fn step_style(&mut self,delta:isize,cx:&mut Context<Self>){let count=self.catalog.as_ref().and_then(|c|c["styles"].as_array()).map(Vec::len).unwrap_or(0);if count>0{let index=(self.style_index as isize+delta).rem_euclid(count as isize)as usize;self.choose_style(index,cx);}}
    fn set_group(&mut self,group:&str,cx:&mut Context<Self>){if let Some(id)=self.selected.clone(){self.preferences.projects.entry(id).or_default().group=group.into();self.persist();self.rebuild_tiles(cx);}}
    fn set_names(&mut self,cx:&mut Context<Self>){self.preferences.named=!self.preferences.named;self.persist();self.rebuild_tiles(cx);}
    fn set_view(&mut self,mode:&str,cx:&mut Context<Self>){self.preferences.view=mode.into();self.focused=false;self.viewport.reset();if mode=="roam"{self.viewport.zoom=0.62;}self.stop_playback(cx);self.persist();cx.notify();}
    fn select_tile(&mut self,index:usize,focus:bool,cx:&mut Context<Self>){if index>=self.tiles.len(){return;}self.stop_playback(cx);self.selected_tile=Some(index);self.focused=focus;self.viewport.reset();self.hover=None;self.panel=None;cx.notify();}
    fn select_piece(&mut self,piece:String,cx:&mut Context<Self>){if let Some(index)=self.tiles.iter().position(|t|t.spec.piece==piece){self.select_tile(index,true,cx);}else if let Some(def)=self.piece(&piece){let variant=default_variant(def);self.tiles.push(Tile{spec:TileSpec{key:format!("{}#{}",piece,variant.as_deref().unwrap_or("")),piece,variant,span:1},art:None,image:None,pixels:0,error:None,loading:false});self.select_tile(self.tiles.len()-1,true,cx);}}
    fn choose_variant(&mut self,variant:String,cx:&mut Context<Self>){let Some(tile)=self.current_tile()else{return;};let key=tile.spec.key.clone();let specs=self.tiles.iter().map(|t|t.spec.clone()).collect::<Vec<_>>();let Some(id)=self.selected.clone()else{return;};let prefs=self.preferences.projects.entry(id).or_default();if swap_variant(&specs,&mut prefs.variants,&key,&variant){self.persist();let selected=self.selected_tile;let focus=self.focused;self.rebuild_tiles(cx);self.selected_tile=selected;self.focused=focus;}}
    fn move_piece(&mut self,delta:isize,cx:&mut Context<Self>){if self.tiles.is_empty(){return;}let next=(self.selected_tile.unwrap_or(0)as isize+delta).rem_euclid(self.tiles.len()as isize)as usize;self.select_tile(next,self.focused,cx);}
    fn zoom(&mut self,factor:f32,cx:&mut Context<Self>){if !self.focused&&self.preferences.view!="roam"{self.set_view("roam",cx);}self.viewport.zoom_at(factor,self.viewport_size.0/2.,self.viewport_size.1/2.);cx.notify();}
    fn toggle_language(&mut self,_:&ToggleLanguage,_:&mut Window,cx:&mut Context<Self>){self.preferences.language=match self.preferences.language{Language::Zh=>Language::En,Language::En=>Language::Zh};self.persist();install_menus(&self.preferences.language,cx);self.rebuild_tiles(cx);}
    fn cycle_appearance(&mut self,_:&CycleAppearance,window:&mut Window,cx:&mut Context<Self>){self.preferences.appearance=match self.preferences.appearance{Appearance::System=>Appearance::Light,Appearance::Light=>Appearance::Dark,Appearance::Dark=>Appearance::System};apply_appearance(&self.preferences.appearance,window,cx);self.persist();cx.notify();}
    fn toggle_motion(&mut self,_:&ToggleMotion,_:&mut Window,cx:&mut Context<Self>){self.preferences.reduced_motion=!self.preferences.reduced_motion;cx.set_reduce_motion(self.preferences.reduced_motion);self.stop_playback(cx);self.persist();cx.notify();}
    fn play(&mut self,restart:bool,cx:&mut Context<Self>){
        if let Some(play)=&mut self.playback{if restart{play.elapsed=Duration::ZERO;play.index=0;play.started=Instant::now();play.playing=true;}else if play.playing{play.elapsed=play.started.elapsed();play.playing=false;}else{play.started=Instant::now()-play.elapsed;play.playing=true;}cx.notify();return;}
        let Some(tile)=self.current_tile()else{return;};if self.piece(&tile.spec.piece).is_none_or(|p|p["animated"]!=true){return;}let Some(mut request)=self.artwork_request(&tile.spec,"frames")else{return;};let selected=self.selected_tile;let generation=self.preview_generation;request["frameCount"]=json!(36);request["reducedMotion"]=json!(self.preferences.reduced_motion);
        self.focused=true;self.viewport.reset();self.animation_busy=true;let cancellation=renderer::Cancellation::default();self.animation_cancellation=Some(cancellation.clone());
        self.animation_task=Some(cx.spawn(async move|this,cx|{let result:Result<Playback,String>=cx.background_spawn(async move{let result=renderer::request(request,cancellation.clone())?;let arts:Vec<Artwork>=serde_json::from_value(result["frames"].clone()).map_err(|e|e.to_string())?;let times:Vec<f64>=serde_json::from_value(result["times"].clone()).map_err(|e|e.to_string())?;let timeline:Timeline=serde_json::from_value(result["timeline"].clone()).map_err(|e|e.to_string())?;if arts.len()!=times.len()||arts.is_empty(){return Err("Invalid animation frames".into());}
            let ratio=arts.iter().map(|a|(a.h/a.w)as f64).sum::<f64>().max(1.);let width=(96.*1024.*1024./(ratio*4.)).sqrt().min(1024.).max(128.)as u32;let mut frames=Vec::new();for art in arts{if cancellation.cancelled(){return Err("Animation cancelled".into());}let raster=native_preview::rasterize(&art.svg,width,&cancellation)?;frames.push((art,Arc::new(Image::from_bytes(ImageFormat::Png,raster.png.clone()))));}Ok(Playback{frames,times,timeline,started:Instant::now(),elapsed:Duration::ZERO,playing:true,index:0})}).await;
            let _=this.update(cx,|view,cx|{if view.preview_generation!=generation||view.selected_tile!=selected{return;}view.animation_busy=false;view.animation_cancellation=None;match result{Ok(play)=>view.playback=Some(play),Err(error)=>view.error=Some(error)}cx.notify();});}));cx.notify();
    }
    pub fn fixture_images_ready(&self,window:&mut Window,cx:&mut App)->bool{
        let visible=Rect{x:0.,y:0.,w:self.viewport_size.0,h:self.viewport_size.1};
        let images=self.tiles.iter().enumerate().filter(|(i,_)|self.screen_rect(*i).intersects(visible)).filter_map(|(_,tile)|tile.image.as_ref()).collect::<Vec<_>>();
        !images.is_empty()&&images.into_iter().all(|image|image.clone().get_render_image(window,cx).is_some())
    }
    fn start_background(&mut self,cx:&mut Context<Self>){
        let(tx,rx)=async_channel::bounded(64);let sink:service::ProgressSink=Arc::new(move|event|{let _=tx.try_send(event);});self.background=Some(service::BackgroundService::start(sink));
        self.background_task=Some(cx.spawn(async move|this,cx|{while let Ok(event)=rx.recv().await{if this.update(cx,|view,cx|{match event["kind"].as_str().unwrap_or(""){
            "archive-changed"=>{if event["projectId"].as_str()==view.selected.as_deref(){if let Some(id)=view.selected.clone(){view.select(id,cx);}}},
            "analysis-started"=>{view.job_busy=true;view.progress=Some(json!({"stage":"history"}));},
            "analysis-progress"=>view.progress=Some(event["progress"].clone()),
            "analysis-finished"=>{view.job_busy=false;view.progress=None;view.refresh(cx);},
            "collect-done"=>view.collect=event["result"].clone(),
            "analysis-interrupted"=>view.notice=Some(view.text("上次分析被中断，可重新分析以继续。","The previous analysis was interrupted. Analyze again to resume.").into()),
            "error"=>view.error=Some(event["error"].to_string()),_=>{}}cx.notify();}).is_err(){break;}}}));
    }
    fn open_panel(&mut self,panel:Panel,window:&mut Window,cx:&mut Context<Self>){
        self.panel=if self.panel==Some(panel){None}else{Some(panel)};
        if panel==Panel::Project&&self.panel.is_some(){let sign=self.details.as_ref().and_then(|d|d["config"]["sign"].as_str()).unwrap_or("").to_string();self.sign_input=cx.new(|cx|InputState::new(window,cx).default_value(sign));self.fetch_details(cx);}
        if panel==Panel::Settings&&!self.fixture{self.collect=service::collect_info();}
        cx.notify();
    }
    fn fetch_details(&mut self,cx:&mut Context<Self>){if self.fixture{return;}let Some(id)=self.selected.clone().filter(|id|id!="__all__")else{return;};let selection=id.clone();self.operation_task=Some(cx.spawn(async move|this,cx|{let result=cx.background_spawn(async move{service::project_details(id)}).await;let _=this.update(cx,|view,cx|{if view.selected.as_ref()!=Some(&selection){return;}match result{Ok(details)=>view.details=Some(details),Err(error)=>view.error=Some(error.to_string())}cx.notify();});}));}
    fn operation(&mut self,work:impl FnOnce()->Result<Value,service::Fail>+Send+'static,cx:&mut Context<Self>){if self.fixture||self.operation_busy{return;}self.operation_busy=true;self.error=None;self.operation_task=Some(cx.spawn(async move|this,cx|{let result=cx.background_spawn(async move{work()}).await;let _=this.update(cx,|view,cx|{view.operation_busy=false;match result{Ok(_)=>{view.collect=service::collect_info();view.notice=Some(view.text("已更新","Updated").into());view.fetch_details(cx);},Err(error)=>view.error=Some(error.to_string())}cx.notify();});}));cx.notify();}
    fn save_project_display(&mut self,cx:&mut Context<Self>){let Some(id)=self.selected.clone()else{return;};let sign=self.sign_input.read(cx).value().to_string();let hidden=!self.preferences.named;self.operation(move||service::set_display(id,hidden,sign).map(|_|Value::Null),cx);}
    fn set_integration(&mut self,which:&str,on:bool,cx:&mut Context<Self>){let Some(id)=self.selected.clone()else{return;};let which=which.to_string();let lang=self.preferences.language.code().to_string();self.operation(move||service::set_integration(id,which,on,Some(lang)),cx);}
    fn remove_project(&mut self,cx:&mut Context<Self>){if self.fixture{return;}let Some(id)=self.selected.clone()else{return;};self.panel=None;self.operation_busy=true;self.operation_task=Some(cx.spawn(async move|this,cx|{let result=cx.background_spawn(async move{service::remove_project(id)}).await;let _=this.update(cx,|view,cx|{view.operation_busy=false;match result{Ok(())=>{view.selected=None;view.raw=None;view.refresh(cx);},Err(error)=>view.error=Some(error.to_string())}cx.notify();});}));}
    fn link_folder(&mut self,cx:&mut Context<Self>){let Some(id)=self.selected.clone()else{return;};let lang=self.preferences.language.code().to_string();let prompt=cx.prompt_for_paths(PathPromptOptions{files:false,directories:true,multiple:false,prompt:Some(self.text("关联项目路径","Link project path").into())});self.dialog_task=Some(cx.spawn(async move|this,cx|{let choice=prompt.await;let _=this.update(cx,|view,cx|{if let Ok(Ok(Some(paths)))=choice{if let Some(path)=paths.first(){let path=path.to_string_lossy().into_owned();view.operation(move||service::link_path(id,path,Some(false),Some(lang)),cx);}}cx.notify();});}));}
    fn unlink_path(&mut self,path:String,cx:&mut Context<Self>){let Some(id)=self.selected.clone()else{return;};self.operation(move||service::link_path(id,path,Some(true),None),cx);}
    fn show_all_projects(&mut self,cx:&mut Context<Self>){if self.fixture{return;}self.clear_preview(cx);self.archive_generation=self.archive_generation.wrapping_add(1);let generation=self.archive_generation;self.selected=Some("__all__".into());self.preferences.last_project=self.selected.clone();self.loading=true;self.panel=None;self.raw=None;self.archive_task=Some(cx.spawn(async move|this,cx|{let result:Result<(Value,Vec<Value>),String>=cx.background_spawn(async move{let collection=service::project_archives().map_err(|e|e.to_string())?;let projects=collection.as_array().into_iter().flatten().filter_map(|p|p.get("raw").filter(|v|v.is_object()).cloned()).collect::<Vec<_>>();if projects.is_empty(){return Err("No project archives are available".into());}let result=renderer::request(json!({"version":1,"id":"all-projects","op":"merge","projects":projects}),renderer::Cancellation::default())?;Ok((result["raw"].clone(),projects))}).await;let _=this.update(cx,|view,cx|{if view.archive_generation!=generation{return;}view.loading=false;match result{Ok((raw,projects))=>{view.raw=Some(raw);view.all_archives=projects;view.load_catalog(cx);},Err(error)=>view.error=Some(error)}cx.notify();});}));}
    fn selected_piece(&self)->Option<&Value>{self.current_tile().and_then(|t|self.piece(&t.spec.piece))}
    fn can_export(&self)->bool{self.raw.is_some()&&self.current_tile().is_some()&&self.export_stage.is_none()&&(self.export_format!="gif"||self.selected_piece().is_some_and(|p|p["animated"]==true))}
    fn export_request(&self)->Result<Value,String>{
        let raw=self.raw.as_ref().ok_or("No archive")?;let catalog=self.catalog.as_ref().ok_or("No artwork catalog")?;let tile=self.current_tile().ok_or("No artwork selected")?;
        let pieces=self.style().and_then(|s|s["pieces"].as_array()).ok_or("No pieces")?;let piece_index=pieces.iter().position(|p|p["id"]==tile.spec.piece).ok_or("No piece")?;
        let variant_index=pieces[piece_index]["variants"].as_array().and_then(|vs|vs.iter().position(|v|v["id"].as_str()==tile.spec.variant.as_deref())).unwrap_or(0);
        let mut snapshot=export_with_preferences(raw,catalog,(self.style_index,piece_index,variant_index),&self.preferences,&self.export_format,&self.export_size,self.transparent,&self.font_mode)?;
        if !self.all_archives.is_empty(){snapshot["projects"]=json!(self.all_archives);}Ok(snapshot)
    }
    fn export_artwork(&mut self, _: &ExportArtwork, _: &mut Window, cx: &mut Context<Self>) {
        if !self.can_export() { return; }
        let snapshot = match self.export_request() {
            Ok(snapshot) => snapshot,
            Err(error) => { self.error = Some(error); cx.notify(); return; }
        };
        let directory = match service::export_dir() {
            Ok(directory) => directory,
            Err(error) => { self.error = Some(error.to_string()); cx.notify(); return; }
        };
        let filename = export_filename(&snapshot);
        self.export_label = filename.clone();
        let format = self.export_format.clone();
        let prompt = cx.prompt_for_new_path(&directory, Some(&filename));
        let cancellation = renderer::Cancellation::default();
        self.export_cancellation = Some(cancellation.clone());
        self.export_stage = Some(ExportStage::ChoosePath);
        self.export_cancel_requested = false;
        self.error = None;
        self.notice = None;
        self.export_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let path = match choice {
                Ok(Ok(Some(path))) => path,
                other => {
                    let _ = this.update(cx, |view, cx| {
                        view.export_stage = None;
                        view.export_cancellation = None;
                        match other {
                            Ok(Ok(None)) => {},
                            Ok(Err(error)) => view.error = Some(error.to_string()),
                            Err(error) => view.error = Some(error.to_string()),
                            Ok(Ok(Some(_))) => unreachable!(),
                        }
                        cx.notify();
                    });
                    return;
                }
            };
            let Ok(proceed) = this.update(cx, |view, cx| {
                if view.export_cancel_requested {
                    view.export_stage = None;
                    view.export_cancellation = None;
                    view.notice = Some(view.text("已取消导出", "Export cancelled").into());
                    cx.notify();
                    return false;
                }
                view.export_stage = Some(ExportStage::Rendering);
                cx.notify();
                true
            }) else { cancellation.cancel(); return; };
            if !proceed { return; }
            let render_cancellation = cancellation.clone();
            let result = cx.background_spawn(async move {
                let output = renderer::request(snapshot, render_cancellation)?;
                renderer::decode_export(&output, &format)
            }).await;
            let result = match result {
                Ok(bytes) => {
                    if this.update(cx, |view, cx| {
                        view.export_stage = Some(ExportStage::Writing);
                        cx.notify();
                    }).is_err() { cancellation.cancel(); return; }
                    let output_path = path.clone();
                    cx.background_spawn(async move { renderer::write_export(&output_path, &bytes, cancellation) }).await
                }
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |view, cx| {
                view.export_stage = None;
                view.export_cancellation = None;
                match result {
                    // A successful atomic commit remains success if cancellation
                    // happened just after the final replacement.
                    Ok(()) => view.notice = Some(format!("{} {}", view.text("已保存", "Saved"), path.display())),
                    Err(_) if view.export_cancel_requested => view.notice = Some(view.text("已取消导出", "Export cancelled").into()),
                    Err(error) => view.error = Some(error),
                }
                view.export_cancel_requested = false;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn cancel_export(&mut self, _: &CancelExport, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(cancellation) = &self.export_cancellation {
            cancellation.cancel();
            self.export_cancel_requested = true;
            cx.notify();
        }
    }

    fn cancel_current(&mut self, _: &CancelCurrent, window: &mut Window, cx: &mut Context<Self>) {
        if self.export_stage.is_some() { self.cancel_export(&CancelExport, window, cx); }
        else if self.panel.take().is_some() { self.focus.focus(window,cx); cx.notify(); }
        else if self.focused { self.focused=false;self.viewport.reset();self.stop_playback(cx);cx.notify(); }
        else if self.export_open { self.export_open=false;cx.notify(); }
        else { self.cancel(&CancelAnalysis, window, cx); }
    }    fn copy_export(&mut self,cx:&mut Context<Self>){let mut request=match self.export_request(){Ok(value)=>value,Err(error)=>{self.error=Some(error);return;}};let format=if ["svg","html","md","txt","json","embed"].contains(&self.export_format.as_str()){self.export_format.clone()}else{"svg".into()};request["format"]=json!(format);self.operation_busy=true;self.operation_task=Some(cx.spawn(async move|this,cx|{let result=cx.background_spawn(async move{let output=renderer::request(request,renderer::Cancellation::default())?;let bytes=renderer::decode_export(&output,&format)?;String::from_utf8(bytes).map_err(|e|e.to_string())}).await;let _=this.update(cx,|view,cx|{view.operation_busy=false;match result{Ok(text)=>{cx.write_to_clipboard(ClipboardItem::new_string(text));view.notice=Some(view.text("已复制","Copied").into());},Err(error)=>view.error=Some(error)}cx.notify();});}));}
    fn wheel(&mut self,event:&ScrollWheelEvent,cx:&mut Context<Self>){let delta=event.delta.pixel_delta(px(24.));let dx:f32=delta.x.into();let dy:f32=delta.y.into();if event.modifiers.control||event.modifiers.platform{self.viewport.zoom_at((-dy*0.006).exp(),self.pointer.0,self.pointer.1);}else{self.viewport.y+=dy;if self.preferences.view=="roam"||self.focused{self.viewport.x+=dx;}}self.hover=None;cx.notify();cx.stop_propagation();}
    fn pointer_move(&mut self,event:&MouseMoveEvent,cx:&mut Context<Self>){let x:f32=event.position.x.into();let y:f32=event.position.y.into();let local=(x-220.,y-104.);self.pointer=local;
        if let Some(drag)=&mut self.drag{if event.pressed_button==Some(MouseButton::Left){let dx=local.0-drag.last.0;let dy=local.1-drag.last.1;if (local.0-drag.start.0).abs()+(local.1-drag.start.1).abs()>6.{drag.moved=true;}if drag.tile.is_none()&&(self.preferences.view=="roam"||self.focused){self.viewport.x+=dx;self.viewport.y+=dy;}drag.last=local;self.hover=None;cx.notify();return;}else{self.drag=None;}}
        let mut found=None;for(index,tile)in self.tiles.iter().enumerate(){if self.focused&&self.selected_tile!=Some(index){continue;}let rect=self.screen_rect(index);if !rect.contains(local.0,local.1){continue;}let art=if self.focused{self.playback.as_ref().and_then(|p|p.frames.get(p.index)).map(|f|&f.0).or(tile.art.as_ref())}else{tile.art.as_ref()};if let Some(art)=art{let ax=(local.0-rect.x)*art.w/rect.w;let ay=(local.1-rect.y)*art.h/rect.h;found=art.hits.iter().find(|h|Rect{x:h.x,y:h.y,w:h.w,h:h.h}.contains(ax,ay)).cloned();}break;}
        if found.as_ref().map(|h|(&h.t,&h.d))!=self.hover.as_ref().map(|h|(&h.t,&h.d)){self.hover=found;cx.notify();}
    }
    fn pointer_up(&mut self,event:&MouseUpEvent,cx:&mut Context<Self>){let Some(drag)=self.drag.take()else{return;};if !drag.moved{return;}let Some(from)=drag.tile else{return;};let x:f32=event.position.x.into();let y:f32=event.position.y.into();let local=(x-220.,y-104.);
        if local.1>=self.viewport_size.1-12.{self.selected_tile=Some(from);self.export_open=true;cx.notify();return;}
        let to=self.rects.iter().enumerate().find(|(i,_)|self.screen_rect(*i).contains(local.0,local.1)).map(|(i,_)|i);if let Some(to)=to.filter(|to|*to!=from){let item=self.tiles.remove(from);self.tiles.insert(to,item);self.selected_tile=Some(to);if let Some(id)=self.selected.clone(){self.preferences.projects.entry(id).or_default().order=self.tiles.iter().map(|t|t.spec.key.clone()).collect();self.persist();}cx.notify();}
    }
}
impl Drop for Creditia{fn drop(&mut self){if let Some(c)=&self.analysis_cancellation{c.cancel();}if let Some(c)=&self.preview_cancellation{c.cancel();}if let Some(c)=&self.animation_cancellation{c.cancel();}if let Some(c)=&self.export_cancellation{c.cancel();}}}
impl Creditia{
    fn artwork_element(&self,index:usize,cx:&mut Context<Self>)->AnyElement{
        let tile=&self.tiles[index];let r=self.screen_rect(index);let selected=self.selected_tile==Some(index);let border=cx.theme().border;let muted=cx.theme().muted_foreground;
        let title=self.piece(&tile.spec.piece).and_then(|p|p[if self.preferences.language==Language::En{"en"}else{"name"}].as_str()).unwrap_or(&tile.spec.piece).to_string();
        let image=if self.focused{self.playback.as_ref().and_then(|p|p.frames.get(p.index)).map(|f|f.1.clone()).or(tile.image.clone())}else{tile.image.clone()};
        let mut picture=div().id(("artwork",index)).test_support().relative().w(px(r.w)).h(px(r.h)).overflow_hidden().rounded_sm().cursor_pointer()
            .on_mouse_down(MouseButton::Left,cx.listener(move|view,event:&MouseDownEvent,_,cx|{view.selected_tile=Some(index);view.drag=Some(DragState{tile:if view.focused{None}else{Some(index)},start:view.pointer,last:view.pointer,moved:false});if event.click_count==2{view.select_tile(index,true,cx);}cx.stop_propagation();cx.notify();}));
        if let Some(image)=image{picture=picture.child(img(image).id(("artwork-image",index)).size_full().object_fit(ObjectFit::Contain).with_fallback(||div().child("Unable to decode artwork").into_any_element()));}
        else{picture=picture.bg(cx.theme().muted).flex().items_center().justify_center().child(div().text_sm().text_color(muted).child(if tile.error.as_deref()==Some("__empty__"){self.text("暂无相应数据","No data for this piece").to_string()}else if let Some(error)=&tile.error{error.clone()}else{self.text("正在排版…","Preparing artwork…").to_string()}));}
        let mut card=div().absolute().left(px(r.x)).top(px(r.y)).w(px(r.w)).child(picture);
        if !self.focused{
            let variant=tile.spec.variant.as_deref().unwrap_or("").to_string();
            card=card.child(div().flex().items_center().justify_between().gap_2().h(px(46.)).text_sm().text_color(muted)
                .child(div().flex().gap_2().child(title).child(variant))
                .child(div().flex().gap_1()
                    .child(Button::new(("tile-focus",index)).ghost().label(self.text("放大","Open")).on_click(cx.listener(move|view,_,_,cx|view.select_tile(index,true,cx))))
                    .child(Button::new(("tile-export",index)).ghost().label(self.text("导出","Export")).on_click(cx.listener(move|view,_,_,cx|{view.selected_tile=Some(index);view.export_open=true;cx.notify();})))));
        }
        if selected{card=card.border_1().border_color(border);}
        card.into_any_element()
    }
    fn render_stage(&self,cx:&mut Context<Self>)->AnyElement{
        let visible=Rect{x:-50.,y:-80.,w:self.viewport_size.0+100.,h:self.viewport_size.1+160.};let mut stage=div().id("artwork-stage").test_support().relative().size_full().overflow_hidden()
            .on_scroll_wheel(cx.listener(|view,event,_,cx|view.wheel(event,cx)))
            .on_mouse_down(MouseButton::Left,cx.listener(|view,_,_,cx|{view.drag=Some(DragState{tile:None,start:view.pointer,last:view.pointer,moved:false});view.hover=None;cx.notify();}));
        if self.loading{stage=stage.child(div().absolute().inset_0().flex().items_center().justify_center().child(self.text("正在读取作品库…","Loading project library…")));}
        else if self.raw.is_none(){let empty=div().flex().flex_col().items_center().justify_center().gap_4().size_full()
            .child(div().text_3xl().child("* /creditia"))
            .child(div().text_color(cx.theme().muted_foreground).child(if self.selected.is_some(){self.text("分析项目，生成它的作品。","Analyze this project to create its artwork.")}else{self.text("代码背后的工具与模型，成为一面作品墙。","The tools and models behind your code, on a wall of artwork.")}))
            .child(Button::new("first-project").label(if self.selected.is_some(){self.text("分析项目","Analyze project")}else{self.text("添加项目…","Add project…")}).on_click(cx.listener(|view,_,window,cx|{if view.selected.is_some(){view.start_job(None,cx)}else{view.open_project(&OpenProject,window,cx)}})));
            stage=stage.child(empty);
        }else{
            for index in 0..self.tiles.len(){if (!self.focused||self.selected_tile==Some(index))&&self.screen_rect(index).intersects(visible){stage=stage.child(self.artwork_element(index,cx));}}
        }
        if let Some(hit)=&self.hover{let w=300.;let h=54.+hit.d.len().min(8)as f32*22.;let x=if self.pointer.0+20.+w>self.viewport_size.0{self.pointer.0-w-14.}else{self.pointer.0+20.}.max(8.);let y=if self.pointer.1+20.+h>self.viewport_size.1{self.pointer.1-h-12.}else{self.pointer.1+20.}.max(8.);stage=stage.child(div().id("evidence-tooltip").absolute().left(px(x)).top(px(y)).w(px(w)).p_3().rounded_lg().bg(cx.theme().foreground).text_color(cx.theme().background).text_sm().child(div().font_weight(FontWeight::SEMIBOLD).child(hit.t.clone())).children(hit.d.iter().take(8).map(|line|div().mt_1().child(line.clone()))));}
        stage.into_any_element()
    }
    fn render_export_dock(&self,cx:&mut Context<Self>)->AnyElement{
        let border=cx.theme().border;let muted=cx.theme().muted_foreground;let mut dock=div().id("export-dock").flex().flex_col().gap_2().px_5().py_3().border_t_1().border_color(border);
        if !self.export_open{return dock.h(px(40.)).flex_row().items_center().justify_between().child(div().text_sm().text_color(muted).child(self.text("拖动作品到这里导出","Drag an artwork here to export"))).child(Button::new("open-export-dock").ghost().label(self.text("导出坞","Exports")).on_click(cx.listener(|view,_,_,cx|{view.export_open=true;cx.notify();}))).into_any_element();}
        let formats=self.catalog.as_ref().and_then(|c|c["formats"].as_array()).cloned().unwrap_or_default();
        let mut buttons=div().flex().flex_wrap().gap_1();for(index,format)in formats.iter().enumerate(){let Some(id)=format["id"].as_str()else{continue};let id=id.to_string();let chosen=self.export_format==id;let disabled=id=="gif"&&self.selected_piece().is_none_or(|p|p["animated"]!=true);buttons=buttons.child(Button::new(("format",index)).ghost().label(id.to_uppercase()).when(chosen,|button|button.bg(cx.theme().muted)).disabled(disabled).on_click(cx.listener(move|view,_,_,cx|{view.export_format=id.clone();view.preferences.export_format=id.clone();view.persist();cx.notify();})));}
        let mut sizes=div().flex().flex_wrap().gap_1();for(index,size)in self.catalog.as_ref().and_then(|c|c["sizes"].as_array()).into_iter().flatten().enumerate(){let Some(id)=size["id"].as_str()else{continue};let id=id.to_string();let label=if id=="native"{self.text("原尺寸","Original").to_string()}else{format!("{} {}×{}",id,size["w"],size["h"])};let chosen=self.export_size==id;sizes=sizes.child(Button::new(("size",index)).ghost().label(label).when(chosen,|b|b.bg(cx.theme().muted)).on_click(cx.listener(move|view,_,_,cx|{view.export_size=id.clone();view.preferences.export_size=id.clone();view.persist();cx.notify();})));}
        dock=dock.child(div().flex().items_center().justify_between().child(buttons).child(Button::new("close-export-dock").ghost().label("×").on_click(cx.listener(|view,_,_,cx|{view.export_open=false;cx.notify();}))))
            .child(sizes)
            .child(div().flex().items_center().gap_2()
                .child(Button::new("transparent").ghost().label(if self.transparent{self.text("透明背景 ✓","Transparent ✓")}else{self.text("透明背景","Transparent")}).on_click(cx.listener(|view,_,_,cx|{view.transparent=!view.transparent;cx.notify();})))
                .child(Button::new("font-mode").ghost().label(if self.font_mode=="embed"{self.text("字体嵌入","Embed fonts")}else{self.text("文字转路径","Outline fonts")}).on_click(cx.listener(|view,_,_,cx|{view.font_mode=if view.font_mode=="embed"{"outline"}else{"embed"}.into();cx.notify();})))
                .child(div().flex_1())
                .child(Button::new("copy-export").ghost().label(self.text("复制","Copy")).disabled(!self.can_export()||self.operation_busy).on_click(cx.listener(|view,_,_,cx|view.copy_export(cx))))
                .child(Button::new("save-export").primary().label(self.text("保存…","Save…")).disabled(!self.can_export()).on_click(cx.listener(|view,_,window,cx|view.export_artwork(&ExportArtwork,window,cx)))));
        dock.into_any_element()
    }
    fn render_panel(&self,cx:&mut Context<Self>)->Option<AnyElement>{
        let panel=self.panel?;let title=match panel{Panel::Projects=>self.text("项目作品库","Project library"),Panel::Styles=>self.text("风格","Styles"),Panel::Pieces=>self.text("全部作品","All artworks"),Panel::Settings=>self.text("设置","Settings"),Panel::Project=>self.text("项目设置","Project settings"),Panel::Details=>self.text("归因与证据","Attribution & evidence")};let muted=cx.theme().muted_foreground;
        let mut body=div().flex().flex_col().gap_3();
        match panel{
            Panel::Projects=>{
                let search=self.search_input.read(cx).value().to_lowercase();body=body.child(Input::new(&self.search_input));
                body=body.child(Button::new("all-projects").ghost().label(self.text("全部项目汇总","All projects")).on_click(cx.listener(|view,_,_,cx|view.show_all_projects(cx))));
                for(index,project)in self.projects.iter().enumerate().filter(|(_,p)|search.is_empty()||p.name.to_lowercase().contains(&search)||p.path.to_lowercase().contains(&search)){let id=project.id.clone();body=body.child(Button::new(("choose-project",index)).ghost().label(format!("{}  {}",project.name,if project.exists{""}else{"◌"})).on_click(cx.listener(move|view,_,_,cx|{view.panel=None;view.all_archives.clear();view.select(id.clone(),cx);})));}
                body=body.child(Button::new("add-project").label(self.text("添加文件夹…","Add folder…")).on_click(cx.listener(|view,_,window,cx|view.open_project(&OpenProject,window,cx))));
            }
            Panel::Styles=>{let mut choices=div().flex().flex_wrap().gap_2();for(index,style)in self.catalog.as_ref().and_then(|c|c["styles"].as_array()).into_iter().flatten().enumerate(){let name=style[if self.preferences.language==Language::En{"en"}else{"name"}].as_str().unwrap_or("").to_string();choices=choices.child(Button::new(("style",index)).ghost().label(name).when(index==self.style_index,|b|b.bg(cx.theme().muted)).on_click(cx.listener(move|view,_,_,cx|view.choose_style(index,cx))));}body=body.child(choices);}
            Panel::Pieces=>{let mut choices=div().flex().flex_wrap().gap_2();for(index,piece)in self.style().and_then(|s|s["pieces"].as_array()).into_iter().flatten().enumerate(){let id=piece["id"].as_str().unwrap_or("").to_string();let name=piece[if self.preferences.language==Language::En{"en"}else{"name"}].as_str().unwrap_or(&id).to_string();choices=choices.child(Button::new(("piece",index)).ghost().label(name).on_click(cx.listener(move|view,_,_,cx|view.select_piece(id.clone(),cx))));}body=body.child(choices);}
            Panel::Settings=>{
                body=body.child(Button::new("language").ghost().label(self.text("语言：中文 → English","Language: English → 中文")).on_click(cx.listener(|view,_,window,cx|view.toggle_language(&ToggleLanguage,window,cx))))
                    .child(Button::new("appearance").ghost().label(format!("{}：{:?}",self.text("外观","Appearance"),self.preferences.appearance)).on_click(cx.listener(|view,_,window,cx|view.cycle_appearance(&CycleAppearance,window,cx))))
                    .child(Button::new("reduced-motion").ghost().label(format!("{} {}",self.text("减少动态效果","Reduced motion"),if self.preferences.reduced_motion{"✓"}else{"○"})).on_click(cx.listener(|view,_,window,cx|view.toggle_motion(&ToggleMotion,window,cx))))
                    .child(Button::new("names-settings").ghost().label(format!("{} {}",self.text("显示贡献者姓名","Contributor names"),if self.preferences.named{"✓"}else{"○"})).on_click(cx.listener(|view,_,_,cx|view.set_names(cx))))
                    .child(Button::new("auto-analyze").ghost().label(format!("{} {}",self.text("有新提交时自动分析","Analyze new commits automatically"),if self.preferences.auto_analyze{"✓"}else{"○"})).on_click(cx.listener(|view,_,_,cx|{view.preferences.auto_analyze=!view.preferences.auto_analyze;view.persist();if let Some(bg)=&view.background{bg.refresh();}cx.notify();})))
                    .child(div().mt_3().text_color(muted).child(self.text("后台采集","Background collection")));
                let on=self.collect["on"].as_bool().unwrap_or(false);let agent=self.collect["agent"]["on"].as_bool().unwrap_or(false);let supported=self.collect["agent"]["supported"].as_bool().unwrap_or(false);
                body=body.child(Button::new("collect-toggle").ghost().label(format!("{} {}",self.text("自动采集日志","Collect logs automatically"),if on{"✓"}else{"○"})).disabled(self.operation_busy).on_click(cx.listener(move|view,_,_,cx|view.operation(move||service::set_collect(!on),cx))))
                    .child(Button::new("collect-now").label(self.text("立即采集","Collect now")).disabled(self.operation_busy).on_click(cx.listener(|view,_,_,cx|view.operation(service::collect_now,cx))))
                    .child(Button::new("collect-agent").ghost().label(format!("{} {}",self.text("退出后继续采集","Continue after quitting"),if agent{"✓"}else{"○"})).disabled(!supported||self.operation_busy).on_click(cx.listener(move|view,_,_,cx|view.operation(move||service::set_collect_agent(!agent),cx))))
                    .child(Button::new("export-directory").ghost().label(self.text("更改导出目录…","Change export directory…")).on_click(cx.listener(|view,_,_,cx|view.choose_export_directory(cx))))
                    .child(Button::new("diagnostics").ghost().label(self.text("环境检查","Diagnostics")).on_click(cx.listener(|view,_,_,cx|{view.operation(move||Ok(service::doctor()),cx);})));
            }
            Panel::Project=>{
                if let Some(project)=self.current(){body=body.child(div().text_lg().child(project.name.clone())).child(div().text_sm().text_color(muted).child(project.path.clone()));}
                body=body.child(Input::new(&self.sign_input)).child(Button::new("save-display").label(self.text("保存署名与姓名设置","Save signature and name settings")).disabled(self.operation_busy).on_click(cx.listener(|view,_,_,cx|view.save_project_display(cx))));
                for(which,zh,en)in[("hook","提交时记录证据","Record evidence on commit"),("release","发布时生成作品","Generate artwork on release")]{let on=self.details.as_ref().and_then(|d|d["integrations"][which]["enabled"].as_bool().or_else(||d["integrations"][which]["installed"].as_bool())).unwrap_or(false);body=body.child(Button::new(which).ghost().label(format!("{} {}",self.text(zh,en),if on{"✓"}else{"○"})).disabled(self.operation_busy).on_click(cx.listener(move|view,_,_,cx|view.set_integration(which,!on,cx))));}
                if let Some(details)=&self.details{body=body.child(div().text_sm().text_color(muted).child(format!("{} {} · {} {}",self.text("证据","Evidence"),details["evidence"],self.text("会话","Sessions"),details["sessions"])));for(index,path)in details["paths"].as_array().into_iter().flatten().enumerate(){let path=path["path"].as_str().unwrap_or("").to_string();let unlink=path.clone();body=body.child(div().flex().items_center().gap_2().child(div().flex_1().text_sm().child(path)).child(Button::new(("unlink",index)).ghost().label(self.text("解除关联","Unlink")).on_click(cx.listener(move|view,_,_,cx|view.unlink_path(unlink.clone(),cx)))));}}
                body=body.child(Button::new("link-path").ghost().label(self.text("关联目录…","Link directory…")).on_click(cx.listener(|view,_,_,cx|view.link_folder(cx))))
                    .child(Button::new("reveal").ghost().label(self.text("打开所在位置","Reveal folder")).on_click(cx.listener(|view,_,window,cx|view.reveal_project(&RevealProject,window,cx))))
                    .child(Button::new("remove-project").ghost().label(self.text("从作品库移除（保留证据）","Remove from library (keep evidence)")).disabled(self.operation_busy).on_click(cx.listener(|view,_,_,cx|view.remove_project(cx))));
            }
            Panel::Details=>{if let Some(raw)=&self.raw{body=body.child(div().text_sm().text_color(muted).child(format!("{} {}",self.text("档案生成于","Archive generated"),raw["generatedAt"].as_str().unwrap_or("—"))));for row in crate::contributions(raw){let name=if self.preferences.named{row.name}else{self.text("贡献者","Contributor").to_string()};body=body.child(div().flex().justify_between().child(name).child(format!("{} / {}",row.lines_added,row.verified_lines)));}body=body.child(Button::new("copy-details").label(self.text("复制完整档案","Copy complete archive")).on_click(cx.listener(|view,_,window,cx|view.copy_archive(&CopyArchive,window,cx))));}}
        }
        let modal=div().id("panel").w(px(580.)).max_h(px(680.)).flex().flex_col().p_5().gap_4().bg(cx.theme().background).border_1().border_color(cx.theme().border).rounded_lg()
            .child(div().flex().justify_between().items_center().child(div().text_lg().child(title)).child(Button::new("panel-close").ghost().label("×").on_click(cx.listener(|view,_,window,cx|{view.panel=None;view.focus.focus(window,cx);cx.notify();}))))
            .child(div().id("panel-body").overflow_y_scroll().min_h_0().child(body));
        Some(div().absolute().inset_0().bg(rgba(0x00000066)).flex().items_center().justify_center().child(modal).into_any_element())
    }
    fn choose_export_directory(&mut self,cx:&mut Context<Self>){let prompt=cx.prompt_for_paths(PathPromptOptions{files:false,directories:true,multiple:false,prompt:Some(self.text("导出目录","Export directory").into())});self.dialog_task=Some(cx.spawn(async move|this,cx|{let result=prompt.await;let _=this.update(cx,|view,cx|{if let Ok(Ok(Some(paths)))=result{if let Some(path)=paths.first(){let path=path.to_string_lossy().into_owned();if let Err(error)=service::edit_app_settings(|settings|settings["exportDir"]=json!(path)){view.error=Some(error.to_string());}}}cx.notify();});}));}
}
impl Creditia{
    pub fn diagnostic_state(&self)->Value{json!({"view":self.preferences.view,"focused":self.focused,"group":self.group(),"named":self.preferences.named,"selected_tile":self.selected_tile,"zoom":self.viewport.zoom,"tile_count":self.tiles.len(),"style":self.preferences.style,"playing":self.playback.as_ref().is_some_and(|p|p.playing),"loading":self.loading,"panel":self.panel.map(|p|match p{Panel::Projects=>"projects",Panel::Styles=>"styles",Panel::Pieces=>"pieces",Panel::Settings=>"settings",Panel::Project=>"project",Panel::Details=>"details"})})}
}
impl Render for Creditia{
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement{
        let width:f32=window.viewport_size().width.into();let height:f32=window.viewport_size().height.into();let dock_height=if self.export_open{148.}else{40.};self.compute_layout((width-220.).max(300.),(height-104.-32.-dock_height).max(200.));
        if let Some(background)=&self.background{background.set_active(window.is_window_active());}
        if let Some(play)=&mut self.playback{if play.playing&&self.focused&&window.is_window_active()&&!self.preferences.reduced_motion{play.elapsed=play.started.elapsed();play.index=frame_at(&play.times,&play.timeline,play.elapsed.as_millis()as u64);if !play.timeline.repeat&&play.elapsed.as_millis()as u64>=play.timeline.duration_ms+play.timeline.hold_ms{play.playing=false;}else{window.request_animation_frame();}}else if play.playing&&(!window.is_window_active()||self.preferences.reduced_motion){play.elapsed=play.started.elapsed();play.playing=false;}}
        self.schedule_visible(window.scale_factor(),cx);
        let bg=cx.theme().background;let fg=cx.theme().foreground;let muted=cx.theme().muted_foreground;let border=cx.theme().border;
        let title=if self.selected.as_deref()==Some("__all__"){self.text("全部项目","All projects").to_string()}else{self.current().map(|p|p.name.clone()).unwrap_or_else(||self.text("项目作品库","Project library").into())};
        let style_name=self.style().and_then(|s|s[if self.preferences.language==Language::En{"en"}else{"name"}].as_str()).unwrap_or(self.text("风格","Style")).to_string();
        let mut projects=div().id("project-list").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap_1();for(index,project)in self.projects.iter().enumerate(){let id=project.id.clone();let selected=self.selected.as_ref()==Some(&id);projects=projects.child(Button::new(("project",index)).ghost().w_full().label(project.name.clone()).when(selected,|b|b.bg(cx.theme().muted)).on_click(cx.listener(move|view,_,_,cx|{view.all_archives.clear();view.select(id.clone(),cx);})));}
        let sidebar=div().w(px(220.)).flex_shrink_0().h_full().flex().flex_col().border_r_1().border_color(border).p_4().gap_4()
            .child(div().h(px(40.)).flex().items_center().text_2xl().font_weight(FontWeight::MEDIUM).child("* /creditia"))
            .child(Button::new("switch-project").ghost().label(self.text("切换项目…","Switch project…")).on_click(cx.listener(|view,_,window,cx|view.open_panel(Panel::Projects,window,cx))))
            .child(projects)
            .child(Button::new("all-projects-sidebar").ghost().label(self.text("跨项目汇总","All projects")).on_click(cx.listener(|view,_,_,cx|view.show_all_projects(cx))))
            .child(Button::new("open").ghost().label(self.text("添加项目…","Add project…")).disabled(self.job_busy).on_click(cx.listener(|view,_,window,cx|view.open_project(&OpenProject,window,cx))))
            .child(Button::new("settings").ghost().label(self.text("设置","Settings")).on_click(cx.listener(|view,_,window,cx|view.open_panel(Panel::Settings,window,cx))));
        let toolbar=div().h(px(52.)).flex_shrink_0().flex().items_center().gap_2().px_5().border_b_1().border_color(border)
            .child(div().text_lg().font_weight(FontWeight::MEDIUM).child(title))
            .child(Button::new("project-settings").ghost().label("⋯").disabled(self.selected.is_none()||self.selected.as_deref()==Some("__all__")).on_click(cx.listener(|view,_,window,cx|view.open_panel(Panel::Project,window,cx))))
            .child(div().flex_1())
            .child(Button::new("analyze").ghost().label(self.text("重新分析","Analyze")).disabled(self.job_busy||self.current().is_none_or(|p|!p.exists)).on_click(cx.listener(|view,_,_,cx|view.start_job(None,cx))))
            .child(Button::new("copy").ghost().label(self.text("复制档案","Copy archive")).disabled(self.raw.is_none()).on_click(cx.listener(|view,_,window,cx|view.copy_archive(&CopyArchive,window,cx))))
            .child(Button::new("details").ghost().label(self.text("证据","Evidence")).disabled(self.raw.is_none()).on_click(cx.listener(|view,_,window,cx|view.open_panel(Panel::Details,window,cx))))
            .child(Button::new("wall").ghost().label(self.text("展墙","Wall")).when(self.preferences.view=="wall"&&!self.focused,|b|b.bg(cx.theme().muted)).on_click(cx.listener(|view,_,_,cx|view.set_view("wall",cx))))
            .child(Button::new("roam").ghost().label(self.text("漫游","Roam")).when(self.preferences.view=="roam"&&!self.focused,|b|b.bg(cx.theme().muted)).on_click(cx.listener(|view,_,_,cx|view.set_view("roam",cx))));
        let mut controls=div().id("view-controls").h(px(52.)).flex_shrink_0().flex().items_center().gap_1().px_5().border_b_1().border_color(border)
            .child(Button::new("styles").ghost().label(style_name).on_click(cx.listener(|view,_,window,cx|view.open_panel(Panel::Styles,window,cx))))
            .child(Button::new("pieces").ghost().label(self.text("作品","Artworks")).on_click(cx.listener(|view,_,window,cx|view.open_panel(Panel::Pieces,window,cx))));
        let group=self.group();for(id,label)in[("auto",self.text("自动","Auto")),("tool",self.text("工具","Tools")),("model",self.text("模型","Models"))]{let element=match id{"auto"=>"group-auto","tool"=>"group-tool",_=>"group-model"};controls=controls.child(Button::new(element).ghost().label(label).when(group==id,|b|b.bg(cx.theme().muted)).on_click(cx.listener(move|view,_,_,cx|view.set_group(id,cx))));}
        controls=controls.child(Button::new("names").ghost().label(if self.preferences.named{self.text("姓名 ✓","Names ✓")}else{self.text("姓名 ○","Names ○")}).on_click(cx.listener(|view,_,_,cx|view.set_names(cx)))).child(div().flex_1());
        if self.focused{controls=controls.child(Button::new("previous-piece").ghost().label("←").on_click(cx.listener(|view,_,_,cx|view.move_piece(-1,cx))))
            .child(Button::new("next-piece").ghost().label("→").on_click(cx.listener(|view,_,_,cx|view.move_piece(1,cx))))
            .child(Button::new("play").ghost().label(if self.playback.as_ref().is_some_and(|p|p.playing){self.text("暂停","Pause")}else{self.text("播放","Play")}).disabled(self.animation_busy||self.selected_piece().is_none_or(|p|p["animated"]!=true)).on_click(cx.listener(|view,_,_,cx|view.play(false,cx))))
            .child(Button::new("replay").ghost().label("↻").disabled(self.animation_busy).on_click(cx.listener(|view,_,_,cx|view.play(true,cx))))
            .child(Button::new("focus-close").ghost().label("×").on_click(cx.listener(|view,_,_,cx|{view.focused=false;view.viewport.reset();view.stop_playback(cx);cx.notify();})));}
        if self.focused||self.preferences.view=="roam"{controls=controls.child(Button::new("zoom-out").ghost().label("−").on_click(cx.listener(|view,_,_,cx|view.zoom(1./1.2,cx))))
            .child(Button::new("zoom-reset").ghost().label(format!("{}%",(self.viewport.zoom*100.).round()as i32)).on_click(cx.listener(|view,_,_,cx|{view.viewport.reset();cx.notify();})))
            .child(Button::new("zoom-in").ghost().label("+").on_click(cx.listener(|view,_,_,cx|view.zoom(1.2,cx))));}
        let mut status=div().h(px(32.)).flex_shrink_0().flex().items_center().gap_2().px_5().border_t_1().border_color(border).text_xs().text_color(muted);
        if let Some(stage)=self.export_stage{let label=match stage{ExportStage::ChoosePath=>self.text("选择保存位置…","Choose a save location…"),ExportStage::Rendering=>self.text("正在导出…","Exporting…"),ExportStage::Writing=>self.text("正在保存…","Saving…")};status=status.child(format!("{} · {}",label,self.export_label)).child(Button::new("cancel-export").ghost().label(self.text("取消","Cancel")).on_click(cx.listener(|view,_,window,cx|view.cancel_export(&CancelExport,window,cx))));}
        else if self.job_busy{status=status.child(self.progress.as_ref().map(|p|progress_text(p,&self.preferences.language)).unwrap_or_else(||self.text("正在分析…","Analyzing…").into())).child(Button::new("cancel-analysis").ghost().label(self.text("取消","Cancel")).on_click(cx.listener(|view,_,window,cx|view.cancel(&CancelAnalysis,window,cx))));}
        else if let Some(error)=&self.error{status=status.child(error.clone()).child(Button::new("retry").ghost().label(self.text("重试","Retry")).on_click(cx.listener(|view,_,_,cx|{view.error=None;view.refresh(cx);})));}
        else if self.animation_busy{status=status.child(self.text("正在准备动画…","Preparing animation…"));}
        else if let Some(notice)=&self.notice{status=status.child(notice.clone());}
        else{let loaded=self.tiles.iter().filter(|t|t.art.is_some()).count();status=status.child(format!("{} / {} {}",loaded,self.tiles.len(),self.text("件作品","artworks"))).child(div().flex_1()).child(self.text("双击放大 · 拖动调整顺序","Double-click to open · Drag to reorder"));}
        let stage=self.render_stage(cx);let export=self.render_export_dock(cx);let modal=self.render_panel(cx);
        div().relative().size_full().flex().bg(bg).text_color(fg).track_focus(&self.focus).key_context("Creditia")
            .on_action(cx.listener(Self::open_project)).on_action(cx.listener(|view,_:&Refresh,_,cx|view.refresh(cx))).on_action(cx.listener(Self::analyze)).on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::copy_archive)).on_action(cx.listener(Self::reveal_project)).on_action(cx.listener(Self::toggle_language)).on_action(cx.listener(Self::cycle_appearance)).on_action(cx.listener(Self::toggle_motion))
            .on_action(cx.listener(|view,_:&ExportArtwork,_,cx|{view.export_open=true;cx.notify();})).on_action(cx.listener(Self::cancel_export)).on_action(cx.listener(Self::cancel_current))
            .on_action(cx.listener(|view,_:&SwitchProject,window,cx|view.open_panel(Panel::Projects,window,cx))).on_action(cx.listener(|view,_:&Settings,window,cx|view.open_panel(Panel::Settings,window,cx))).on_action(cx.listener(|view,_:&ProjectSettings,window,cx|view.open_panel(Panel::Project,window,cx)))
            .on_action(cx.listener(|view,_:&Wall,_,cx|view.set_view("wall",cx))).on_action(cx.listener(|view,_:&Roam,_,cx|view.set_view("roam",cx))).on_action(cx.listener(|view,_:&PreviousStyle,_,cx|view.step_style(-1,cx))).on_action(cx.listener(|view,_:&NextStyle,_,cx|view.step_style(1,cx)))
            .on_action(cx.listener(|view,_:&ZoomIn,_,cx|view.zoom(1.2,cx))).on_action(cx.listener(|view,_:&ZoomOut,_,cx|view.zoom(1./1.2,cx))).on_action(cx.listener(|view,_:&ResetZoom,_,cx|{view.viewport.reset();cx.notify();}))
            .on_action(cx.listener(|view,_:&Play,_,cx|view.play(false,cx))).on_action(cx.listener(|view,_:&Replay,_,cx|view.play(true,cx))).on_action(cx.listener(|view,_:&PreviousPiece,_,cx|view.move_piece(-1,cx))).on_action(cx.listener(|view,_:&NextPiece,_,cx|view.move_piece(1,cx)))
            .on_mouse_move(cx.listener(|view,event,_,cx|view.pointer_move(event,cx))).on_mouse_up(MouseButton::Left,cx.listener(|view,event,_,cx|view.pointer_up(event,cx)))
            .child(sidebar).child(div().flex().flex_col().flex_1().min_w_0().child(toolbar).child(controls).child(div().flex_1().min_h_0().child(stage)).child(export).child(status)).children(modal)
    }
}
fn apply_appearance(appearance:&Appearance,window:&mut Window,cx:&mut App){match appearance{Appearance::System=>Theme::sync_system_appearance(Some(window),cx),Appearance::Light=>Theme::change(ThemeMode::Light,Some(window),cx),Appearance::Dark=>Theme::change(ThemeMode::Dark,Some(window),cx)}}
fn install_menus(language:&Language,cx:&mut App){cx.set_menus(vec![
    Menu{name:"Creditia".into(),disabled:false,items:vec![MenuItem::action(language.text("设置…","Settings…"),Settings),MenuItem::action(language.text("切换语言","Switch language"),ToggleLanguage),MenuItem::action(language.text("切换明暗主题","Cycle appearance"),CycleAppearance),MenuItem::action(language.text("减少动态效果","Reduced motion"),ToggleMotion),MenuItem::separator(),MenuItem::action(language.text("退出 Creditia","Quit Creditia"),Quit)]},
    Menu{name:language.text("项目","Project").into(),disabled:false,items:vec![MenuItem::action(language.text("切换项目…","Switch project…"),SwitchProject),MenuItem::action(language.text("添加文件夹…","Add folder…"),OpenProject),MenuItem::action(language.text("重新分析","Re-analyze"),Analyze),MenuItem::action(language.text("取消分析","Cancel analysis"),CancelAnalysis),MenuItem::action(language.text("项目设置…","Project settings…"),ProjectSettings),MenuItem::separator(),MenuItem::action(language.text("复制档案","Copy archive"),CopyArchive),MenuItem::action(language.text("打开所在位置","Reveal folder"),RevealProject)]},
    Menu{name:language.text("编辑","Edit").into(),disabled:false,items:vec![MenuItem::action(language.text("撤销","Undo"),gpui_kit::component::input::Undo),MenuItem::action(language.text("重做","Redo"),gpui_kit::component::input::Redo),MenuItem::separator(),MenuItem::action(language.text("剪切","Cut"),gpui_kit::component::input::Cut),MenuItem::action(language.text("复制","Copy"),gpui_kit::component::input::Copy),MenuItem::action(language.text("粘贴","Paste"),gpui_kit::component::input::Paste),MenuItem::action(language.text("全选","Select all"),gpui_kit::component::input::SelectAll)]},
    Menu{name:language.text("视图","View").into(),disabled:false,items:vec![MenuItem::action(language.text("展墙","Wall"),Wall),MenuItem::action(language.text("漫游","Roam"),Roam),MenuItem::action(language.text("上一种风格","Previous style"),PreviousStyle),MenuItem::action(language.text("下一种风格","Next style"),NextStyle),MenuItem::separator(),MenuItem::action(language.text("放大","Zoom in"),ZoomIn),MenuItem::action(language.text("缩小","Zoom out"),ZoomOut),MenuItem::action(language.text("重置缩放","Reset zoom"),ResetZoom),MenuItem::separator(),MenuItem::action(language.text("播放／暂停","Play / pause"),Play),MenuItem::action(language.text("重播","Replay"),Replay)]},
    Menu{name:language.text("导出","Export").into(),disabled:false,items:vec![MenuItem::action(language.text("导出作品…","Export artwork…"),ExportArtwork),MenuItem::action(language.text("取消导出","Cancel export"),CancelExport)]},
]);}
pub fn bind_keys(cx:&mut App){cx.bind_keys([
    KeyBinding::new("secondary-q",Quit,None),KeyBinding::new("secondary-k",SwitchProject,Some("Creditia")),KeyBinding::new("secondary-o",OpenProject,Some("Creditia")),KeyBinding::new("secondary-r",Analyze,Some("Creditia")),KeyBinding::new("secondary-i",ProjectSettings,Some("Creditia")),KeyBinding::new("secondary-,",Settings,Some("Creditia")),KeyBinding::new("secondary-1",Wall,Some("Creditia")),KeyBinding::new("secondary-2",Roam,Some("Creditia")),KeyBinding::new("secondary-[",PreviousStyle,Some("Creditia")),KeyBinding::new("secondary-]",NextStyle,Some("Creditia")),KeyBinding::new("secondary-e",ExportArtwork,Some("Creditia")),KeyBinding::new("secondary-shift-r",Refresh,Some("Creditia")),KeyBinding::new("secondary-shift-c",CopyArchive,Some("Creditia")),KeyBinding::new("escape",CancelCurrent,Some("Creditia")),KeyBinding::new("secondary-=",ZoomIn,Some("Creditia")),KeyBinding::new("secondary--",ZoomOut,Some("Creditia")),KeyBinding::new("secondary-0",ResetZoom,Some("Creditia")),KeyBinding::new("space",Play,Some("Creditia")),KeyBinding::new("left",PreviousPiece,Some("Creditia")),KeyBinding::new("right",NextPiece,Some("Creditia"))]);}
pub fn run(){
    let args=std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|s|s=="--version"){println!("Creditia {}",env!("CARGO_PKG_VERSION"));return;}
    if args.iter().any(|s|s=="--check-resources"||s=="--self-test"){
        let result=(||->Result<Value,String>{let catalog=renderer::request(json!({"version":1,"id":"resources","op":"catalog"}),renderer::Cancellation::default())?;let core=service::core_bin();let status=std::process::Command::new(&core).arg("--help").output().map_err(|e|e.to_string())?;if !status.status.success(){return Err("Analysis executable failed to launch".into());}Ok(json!({"ok":true,"styles":catalog["styles"].as_array().map(Vec::len),"core":core}))})();match result{Ok(value)=>println!("{value}"),Err(error)=>{eprintln!("{error}");std::process::exit(1);}}return;
    }
    let fixture=args.iter().position(|s|s=="--fixture").and_then(|i|args.get(i+1)).map(|path|std::fs::read(path).map_err(|e|e.to_string()).and_then(|bytes|serde_json::from_slice::<Fixture>(&bytes).map_err(|e|e.to_string())));
    let fixture=match fixture{Some(Ok(fixture))=>Some(fixture),Some(Err(error))=>{eprintln!("Invalid fixture: {error}");std::process::exit(2);},None=>None};
    application().with_assets(assets::Assets).run(move|cx|{init(cx);bind_keys(cx);cx.on_action(|_:&Quit,cx|{service::cancel_analyze();cx.quit();});open_window(WindowOptions{titlebar:Some(TitlebarOptions{title:Some("creditia".into()),..Default::default()}),window_bounds:Some(WindowBounds::Windowed(Bounds::centered(None,size(px(1380.),px(900.)),cx))),window_min_size:Some(size(px(900.),px(620.))),..Default::default()},cx,move|window,cx|cx.new(|cx|match fixture{Some(fixture)=>Creditia::fixture(fixture,window,cx),None=>Creditia::new(window,cx)})).expect("Unable to open Creditia");cx.activate(true);});
}
