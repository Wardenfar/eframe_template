use std::{
    future::Future,
    str::FromStr,
    sync::{Arc, OnceLock},
};

use tokio::{runtime::Runtime, sync::oneshot};

use ethers::{
    core::types::Block,
    types::{BlockId, BlockNumber, H256},
};
use ethers_providers::{Http, Middleware, Provider};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    #[serde(skip)]
    block: AsyncCell<H256, Block<H256>>,
    #[serde(skip)]
    provider: Arc<Provider<Http>>,
}

fn get_runtime() -> Arc<Runtime> {
    static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| Arc::new(Runtime::new().unwrap()))
        .clone()
}

#[derive(Default)]
pub struct AsyncCell<K, T> {
    cache: Option<(K, T)>,
    receiver: Option<(K, oneshot::Receiver<T>)>,
}

impl<K, T> AsyncCell<K, T>
where
    T: Send + 'static,
    K: Eq,
{
    pub fn get_or_update<FB, F>(&mut self, key: K, future_builder: FB) -> Option<&T>
    where
        FB: FnOnce() -> F,
        F: Future<Output = T> + Send + 'static,
    {
        if let Some((cached_key, _)) = &self.cache {
            if cached_key != &key {
                self.cache = None;
            }
        }

        match self.receiver.take() {
            Some((fetching_key, mut receiver)) => {
                if let Ok(value) = receiver.try_recv() {
                    self.cache = Some((fetching_key, value));
                } else {
                    self.receiver = Some((fetching_key, receiver));
                }
            }
            None => {
                let fut = future_builder();
                let runtime = get_runtime();
                let (sender, receiver) = oneshot::channel();
                runtime.spawn(async move { sender.send(fut.await) });
                self.receiver = Some((key, receiver));
            }
        }

        self.cache.as_ref().map(|c| &c.1)
    }
}

impl Default for TemplateApp {
    fn default() -> Self {
        let provider = Provider::<Http>::try_from("https://eth.llamarpc.com")
            .expect("could not instantiate HTTP Provider");

        Self {
            block: Default::default(),
            provider: Arc::new(provider),
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);

                egui::widgets::global_dark_light_mode_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("eframe template");

            ui.separator();

            let hash = H256::from_str(
                "0xf45e2dd95ab165ea215c7c3a5001d7f79f52d5685c18ef54d3d046b773d372f2",
            )
            .unwrap();

            let provider = self.provider.clone();
            let block_opt = self.block.get_or_update(hash, || async move {
                provider
                    .get_block(BlockId::Hash(hash))
                    .await
                    .unwrap()
                    .unwrap()
            });

            if let Some(block) = block_opt {
                ui.label(format!("Trans count : {}", block.transactions.len()));
            } else {
                ui.spinner();
            }

            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/master/",
                "Source code."
            ));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
