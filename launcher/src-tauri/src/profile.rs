//! The hardware profile: « materiel » in series-s/reglages.json turned into changes to the Series S settings for
//! this PC (processor threads, graphics card memory, RAM, vendor). Pure: the same machine always gives the same
//! profile, so the unit tests below cover synthetic machines. series-s/reglages.ps1 (Get-HardwareProfile) does
//! exactly the same for the console launcher; scripts/check-profils.ps1 compares the two.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// What the profile depends on. Zero means unknown (that rule is then left out).
#[derive(Clone, Debug, Default)]
pub struct Hardware {
    pub ram_bytes: u64,
    pub vram_bytes: u64,
    /// Logical processors.
    pub threads: usize,
    pub gpu_name: String,
    /// None: not checked.
    pub avx2: Option<bool>,
}

#[derive(Deserialize, Clone)]
pub struct Tier {
    pub id: String,
    pub nom: String,
    #[serde(default)]
    pub moins_de_fils: Option<usize>,
    #[serde(default)]
    pub moins_de_go: Option<f64>,
    pub env: Map<String, Value>,
}

#[derive(Deserialize, Clone)]
pub struct Tiers {
    pub crans: Vec<Tier>,
}

/// « petite_ram » in reglages.json: under 24 GB, part of the game's memory windows in the card's memory.
#[derive(Deserialize, Clone)]
pub struct LowRam {
    pub note: String,
    pub moins_de_go: f64,
    pub env: Map<String, Value>,
}

#[derive(Deserialize, Clone)]
pub struct VeryLowRam {
    pub moins_de_go: f64,
    pub crans_de_fils_en_moins: usize,
    pub plafonds: Map<String, Value>,
    pub conseil: String,
}

#[derive(Deserialize, Clone)]
pub struct Vendor {
    pub nom: String,
    pub dlss: bool,
    pub shaders_nvidia: bool,
    pub avertissement: String,
}

#[derive(Deserialize, Clone)]
pub struct Materiel {
    pub processeur: Tiers,
    pub carte: Tiers,
    pub tres_petite_ram: VeryLowRam,
    pub fabricants: Map<String, Value>,
    pub sans_avx2: String,
}

/// The profile for one PC.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// « RTX 3050 · 8 Go · 12 fils · 32 Go »
    pub label: String,
    /// nvidia_rtx, nvidia, amd, intel_arc, intel_integree, inconnu.
    pub vendor: String,
    pub vendor_name: String,
    /// Why each setting is what it is, one line per rule.
    pub reasons: Vec<String>,
    /// Things the player should know (untested card, integrated chip, no AVX2).
    pub warnings: Vec<String>,
    /// Advice (16 GB: close the browser).
    pub tips: Vec<String>,
    /// The tiers chosen (« 12_fils », « 8_go », « moins_de_20_go »), for feedback reports.
    pub tiers: Vec<String>,
    /// The variables changed from the groups of reglages.json, in order.
    pub env: Vec<(String, String)>,
    /// DLSS rows offered (NVIDIA RTX).
    pub dlss: bool,
    /// The NVIDIA facts for « victory shaders » (waves of 32...).
    pub shaders_nvidia: bool,
}

/// The vendor class from the card's name (same rules as Get-GpuVendor in reglages.ps1).
pub fn vendor_of(gpu_name: &str) -> &'static str {
    let n = gpu_name.to_uppercase();
    if n.contains("NVIDIA") || n.contains("GEFORCE") {
        if n.contains("RTX") { "nvidia_rtx" } else { "nvidia" }
    } else if n.contains("AMD") || n.contains("RADEON") {
        "amd"
    } else if n.contains("INTEL") {
        // Dedicated Arc cards have a model number (A380, A770, B580); the Arc chips inside processors do not.
        let arc_card = n.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| {
            w.len() == 4 && (w.starts_with('A') || w.starts_with('B')) && w[1..].chars().all(|c| c.is_ascii_digit())
        });
        if n.contains("ARC") && arc_card { "intel_arc" } else { "intel_integree" }
    } else {
        "inconnu"
    }
}

/// "NVIDIA GeForce RTX 3050" -> "RTX 3050" (as shortGpu in the window).
pub fn short_gpu(name: &str) -> String {
    let mut s = name.trim();
    for p in ["NVIDIA ", "AMD ", "Intel(R) "] {
        if s.len() >= p.len() && s[..p.len()].eq_ignore_ascii_case(p) {
            s = &s[p.len()..];
        }
    }
    if s.len() >= 8 && s[..8].eq_ignore_ascii_case("GeForce ") {
        s = &s[8..];
    }
    s.trim().to_string()
}

fn value_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// French number of GB without decimals when whole: 8 -> "8", 15.8 -> "16".
fn go(bytes: u64) -> String {
    format!("{}", (bytes as f64 / GIB).round() as u64)
}

/// Sets `k` in an ordered list: replaced where it is, else added at the end.
pub fn put(list: &mut Vec<(String, String)>, k: &str, v: String) {
    match list.iter_mut().find(|(lk, _)| lk == k) {
        Some(e) => e.1 = v,
        None => list.push((k.to_string(), v)),
    }
}

impl Materiel {
    fn vendor(&self, id: &str) -> Vendor {
        self.fabricants
            .get(id)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(Vendor { nom: id.to_string(), dlss: false, shaders_nvidia: false, avertissement: String::new() })
    }

    /// The profile of a PC. `base` is the groups' environment (to tell what really changes), `small` the
    /// 24 GB rule (« petite_ram »).
    pub fn profile(&self, hw: &Hardware, base: &[(String, String)], small: &LowRam) -> Profile {
        let mut p = Profile::default();
        let mut env: Vec<(String, String)> = Vec::new();
        let ram_gb = hw.ram_bytes as f64 / GIB;
        let vram_gb = hw.vram_bytes as f64 / GIB;
        let very_low_ram = hw.ram_bytes > 0 && ram_gb < self.tres_petite_ram.moins_de_go;

        // Processor: the first tier below its limit; one tier lower on a 16 GB PC.
        let cpu = &self.processeur.crans;
        if hw.threads > 0 && !cpu.is_empty() {
            let found = cpu.iter().position(|t| t.moins_de_fils.is_none_or(|m| hw.threads < m)).unwrap_or(cpu.len() - 1);
            let idx = if very_low_ram { found.saturating_sub(self.tres_petite_ram.crans_de_fils_en_moins) } else { found };
            let t = &cpu[idx];
            p.tiers.push(t.id.clone());
            for (k, v) in &t.env {
                put(&mut env, k, value_str(v));
            }
            let lowered = if idx != found { format!(", un cran plus bas pour {} Go de mémoire vive", go(hw.ram_bytes)) } else { String::new() };
            p.reasons.push(format!(
                "Processeur, {} fils : {}{lowered}, {}.",
                hw.threads,
                t.nom,
                t.env.iter().map(|(k, v)| format!("{}={}", k.trim_start_matches("VICTORY_"), value_str(v))).collect::<Vec<_>>().join(", ")
            ));
        }

        // Graphics card memory.
        let card = &self.carte.crans;
        if hw.vram_bytes > 0 && !card.is_empty() {
            let t = card.iter().find(|t| t.moins_de_go.is_none_or(|m| vram_gb < m)).unwrap_or(&card[card.len() - 1]);
            p.tiers.push(t.id.clone());
            for (k, v) in &t.env {
                put(&mut env, k, value_str(v));
            }
            p.reasons.push(format!(
                "Carte, {} Go : {}, {}.",
                go(hw.vram_bytes),
                t.nom,
                t.env.iter().map(|(k, v)| format!("{}={}", k.trim_start_matches("VICTORY_"), value_str(v))).collect::<Vec<_>>().join(", ")
            ));
        } else {
            p.reasons.push("Carte : mémoire inconnue, réglages pour 8 Go.".into());
        }

        // RAM: under 24 GB the memory windows in the card; under 20 GB also caps (the lower of the card's value
        // and the cap) and the threads one tier lower (above).
        if hw.ram_bytes > 0 {
            let mut parts = Vec::new();
            if ram_gb < small.moins_de_go {
                p.tiers.push(format!("moins_de_{}_go", small.moins_de_go));
                for (k, v) in &small.env {
                    put(&mut env, k, value_str(v));
                }
                parts.push(format!(
                    "moins de {} Go, fenêtres de mémoire du jeu dans la carte ({})",
                    small.moins_de_go,
                    small.env.iter().map(|(k, v)| format!("{}={}", k.trim_start_matches("VICTORY_"), value_str(v))).collect::<Vec<_>>().join(", ")
                ));
            }
            if very_low_ram {
                p.tiers.push(format!("moins_de_{}_go", self.tres_petite_ram.moins_de_go));
                let mut capped = Vec::new();
                for (k, cap) in &self.tres_petite_ram.plafonds {
                    let cap = cap.as_f64().unwrap_or(f64::MAX);
                    let now = env.iter().chain(base.iter()).find(|(ek, _)| ek == k).and_then(|(_, v)| v.parse::<f64>().ok());
                    if now.is_none_or(|n| n > cap) {
                        put(&mut env, k, format!("{cap}"));
                        capped.push(format!("{}={cap}", k.trim_start_matches("VICTORY_")));
                    }
                }
                parts.push(format!(
                    "moins de {} Go, fils un cran plus bas{}",
                    self.tres_petite_ram.moins_de_go,
                    if capped.is_empty() { String::new() } else { format!(" et {}", capped.join(", ")) }
                ));
                p.tips.push(self.tres_petite_ram.conseil.clone());
            }
            p.reasons.push(format!(
                "Mémoire vive, {} Go : {}.",
                go(hw.ram_bytes),
                if parts.is_empty() { "rien à réduire".to_string() } else { parts.join(" ; ") }
            ));
        }

        // Vendor: DLSS, the NVIDIA shader facts, warnings.
        let vendor_id = vendor_of(&hw.gpu_name);
        let v = self.vendor(vendor_id);
        p.vendor = vendor_id.to_string();
        p.vendor_name = v.nom.clone();
        p.dlss = v.dlss;
        p.shaders_nvidia = v.shaders_nvidia;
        if !hw.gpu_name.is_empty() {
            p.reasons.push(format!(
                "{} : {}.",
                v.nom,
                if v.dlss { "DLSS proposé (dossier de Streamline à choisir), « Recommandé » par défaut" } else { "pas de DLSS, 540p et upscaler du jeu" }
            ));
        }
        if !v.avertissement.is_empty() {
            p.warnings.push(v.avertissement.clone());
        }
        if hw.avx2 == Some(false) {
            p.warnings.push(self.sans_avx2.clone());
        }

        // Only what really differs from the groups.
        env.retain(|(k, v)| !base.iter().any(|(bk, bv)| bk == k && bv == v));
        p.env = env;
        let mut parts = Vec::new();
        parts.push(if hw.gpu_name.is_empty() { "carte inconnue".to_string() } else { short_gpu(&hw.gpu_name) });
        if hw.vram_bytes > 0 {
            parts.push(format!("{} Go", go(hw.vram_bytes)));
        }
        if hw.threads > 0 {
            parts.push(format!("{} fils", hw.threads));
        }
        if hw.ram_bytes > 0 {
            parts.push(format!("{} Go de RAM", go(hw.ram_bytes)));
        }
        p.label = parts.join(" · ");
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Reglages, Settings, base_env, game_env_for};

    const GB: u64 = 1 << 30;

    fn reglages() -> Reglages {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../series-s/reglages.json");
        let b = std::fs::read(path).unwrap();
        serde_json::from_slice(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&b)).unwrap()
    }

    fn hw(gpu: &str, vram: u64, threads: usize, ram: u64) -> Hardware {
        Hardware { ram_bytes: ram * GB, vram_bytes: vram * GB, threads, gpu_name: gpu.into(), avx2: Some(true) }
    }

    /// The variables the game gets (without DLSS, VICTORY_CONFIG, the skip list), as NAME=value.
    fn env_of(r: &Reglages, h: &Hardware) -> std::collections::BTreeMap<String, String> {
        let s = Settings(Map::new());
        game_env_for(r, &s, h, "C:\\x\\victory.toml", "skip").set.into_iter().collect()
    }

    fn get<'a>(e: &'a std::collections::BTreeMap<String, String>, k: &str) -> Option<&'a str> {
        e.get(k).map(String::as_str)
    }

    #[test]
    fn vendors() {
        assert_eq!(vendor_of("NVIDIA GeForce RTX 3050"), "nvidia_rtx");
        assert_eq!(vendor_of("NVIDIA GeForce GTX 1660 SUPER"), "nvidia");
        assert_eq!(vendor_of("NVIDIA RTX A2000"), "nvidia_rtx");
        assert_eq!(vendor_of("AMD Radeon RX 6600"), "amd");
        assert_eq!(vendor_of("Intel(R) UHD Graphics 770"), "intel_integree");
        assert_eq!(vendor_of("Intel(R) Arc(TM) Graphics"), "intel_integree");
        assert_eq!(vendor_of("Intel(R) Arc(TM) A770 Graphics"), "intel_arc");
        assert_eq!(vendor_of(""), "inconnu");
        assert_eq!(short_gpu("NVIDIA GeForce RTX 3050"), "RTX 3050");
    }

    /// The reference PC keeps exactly the groups' settings: nothing changed, nothing added.
    #[test]
    fn reference_pc_is_unchanged() {
        let r = reglages();
        let h = hw("NVIDIA GeForce RTX 3050", 8, 12, 32);
        let p = r.materiel.profile(&h, &base_env(&r), &r.petite_ram);
        assert!(p.env.is_empty(), "{:?}", p.env);
        assert!(p.dlss && p.shaders_nvidia && p.warnings.is_empty() && p.tips.is_empty());
        let groups: std::collections::BTreeMap<String, String> = base_env(&r).into_iter().collect();
        let mut e = env_of(&r, &h);
        e.remove("VICTORY_CONFIG");
        e.remove("VICTORY_SKIP_PIPELINES");
        assert_eq!(e, groups);
    }

    /// Five synthetic machines (and a 10 GB card): what each one gets. The same table is in
    /// scripts/check-profils.ps1 for the console launcher.
    #[test]
    fn synthetic_machines() {
        let r = reglages();
        let threads = |e: &std::collections::BTreeMap<String, String>| {
            ["VICTORY_THIN_THREADS", "VICTORY_UNDER_THREADS", "VICTORY_VIEW_THREADS", "VICTORY_PIPELINE_WORKERS"]
                .iter()
                .map(|k| get(e, k).unwrap().to_string())
                .collect::<Vec<_>>()
                .join("/")
        };
        // (machine, threads, CARD_BUFFERS_MB, TEXTURE_MAX, WINDOWS_BAR_MB, dlss, NVIDIA shader facts, warnings)
        type Row<'a> = (Hardware, &'a str, &'a str, Option<&'a str>, Option<&'a str>, bool, bool, usize);
        let rows: [Row; 7] = [
            (hw("NVIDIA GeForce RTX 3050", 8, 12, 32), "4/3/3/4", "1536", None, None, true, true, 0),
            (hw("NVIDIA GeForce GTX 1660 SUPER", 6, 8, 16), "3/2/2/3", "768", Some("2048"), Some("3072"), false, true, 0),
            (hw("NVIDIA GeForce RTX 4050 Laptop GPU", 6, 16, 16), "4/3/3/4", "768", Some("2048"), Some("3072"), true, true, 0),
            (hw("NVIDIA GeForce RTX 4070", 12, 20, 32), "6/4/4/6", "2048", None, None, true, true, 0),
            (hw("AMD Radeon RX 6600", 8, 12, 16), "3/2/2/3", "1024", None, Some("3072"), false, false, 1),
            (hw("Intel(R) UHD Graphics 770", 1, 12, 16), "3/2/2/3", "768", Some("2048"), Some("3072"), false, false, 1),
            (hw("NVIDIA GeForce RTX 3080", 10, 16, 32), "6/4/4/6", "1536", None, None, true, true, 0),
        ];
        for (h, th, buf, tex, bar, dlss, nv, warn) in rows {
            let e = env_of(&r, &h);
            let p = r.materiel.profile(&h, &base_env(&r), &r.petite_ram);
            println!("{} -> threads {} buffers {:?} texture {:?} bar {:?} dlss {} | {}", p.label, threads(&e), get(&e, "VICTORY_CARD_BUFFERS_MB"), get(&e, "VICTORY_TEXTURE_MAX"), get(&e, "VICTORY_WINDOWS_BAR_MB"), p.dlss, p.reasons.join(" "));
            assert_eq!(threads(&e), th, "{}", p.label);
            assert_eq!(get(&e, "VICTORY_CARD_BUFFERS_MB"), Some(buf), "{}", p.label);
            assert_eq!(get(&e, "VICTORY_TEXTURE_MAX"), tex, "{}", p.label);
            assert_eq!(get(&e, "VICTORY_WINDOWS_BAR_MB"), bar, "{}", p.label);
            assert_eq!((p.dlss, p.shaders_nvidia, p.warnings.len()), (dlss, nv, warn), "{}", p.label);
            assert_eq!(p.tips.len(), usize::from(h.ram_bytes < 20 * GB), "{}", p.label);
            // Never 2 GB of card buffers under 11 GB.
            if h.vram_bytes < 11 * GB {
                assert_ne!(get(&e, "VICTORY_CARD_BUFFERS_MB"), Some("2048"), "{}", p.label);
            }
        }
    }

    /// Unknown card memory and threads: the groups' values stay.
    #[test]
    fn unknown_hardware_keeps_the_groups() {
        let r = reglages();
        let p = r.materiel.profile(&Hardware::default(), &base_env(&r), &r.petite_ram);
        assert!(p.env.is_empty());
        let mut h = hw("", 0, 0, 0);
        h.avx2 = Some(false);
        let p = r.materiel.profile(&h, &base_env(&r), &r.petite_ram);
        assert_eq!(p.warnings.len(), 1);
    }
}
