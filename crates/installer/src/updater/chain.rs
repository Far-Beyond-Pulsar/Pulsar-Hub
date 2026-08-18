use crate::updater::manifest::{PatchInfo, PlatformUpdateInfo};

pub const MAX_PATCH_CHAIN_LENGTH: usize = 4;

#[derive(Debug, Clone)]
pub enum UpdateStep {
    Patch {
        from_version: String,
        to_version: String,
        patch_info: PatchInfo,
    },
    FullDownload {
        version: String,
        url: String,
        sha256: String,
        size_bytes: u64,
    },
}

#[derive(Debug)]
pub struct UpdateChain {
    pub steps: Vec<UpdateStep>,
    pub has_full_download: bool,
    pub total_download_bytes: u64,
}

pub fn compute_update_chain(
    from_version: &str,
    to_version: &str,
    platform: &PlatformUpdateInfo,
) -> UpdateChain {
    let mut steps = Vec::new();
    let mut current_version = from_version.to_string();

    loop {
        if current_version == to_version {
            break;
        }

        if steps.len() >= MAX_PATCH_CHAIN_LENGTH {
            tracing::info!(
                "Patch chain exceeded {} hops ({}), falling back to full download",
                MAX_PATCH_CHAIN_LENGTH,
                steps.len()
            );
            return full_download_chain(to_version, platform);
        }

        match platform
            .patches
            .iter()
            .find(|p| p.from_version == current_version && p.to_version == to_version)
        {
            Some(patch) => {
                tracing::info!(
                    "Found patch: {} -> {}",
                    current_version,
                    patch.to_version
                );
                steps.push(UpdateStep::Patch {
                    from_version: current_version.clone(),
                    to_version: patch.to_version.clone(),
                    patch_info: patch.clone(),
                });
                current_version = patch.to_version.clone();
            }
            None => {
                tracing::info!(
                    "No patch from {} to {}, falling back to full download",
                    current_version,
                    to_version
                );
                return full_download_chain(to_version, platform);
            }
        }
    }

    let total_download_bytes = steps
        .iter()
        .map(|step| match step {
            UpdateStep::Patch { patch_info, .. } => patch_info.size_bytes,
            UpdateStep::FullDownload { size_bytes, .. } => *size_bytes,
        })
        .sum();

    UpdateChain {
        steps,
        has_full_download: false,
        total_download_bytes,
    }
}

fn full_download_chain(to_version: &str, platform: &PlatformUpdateInfo) -> UpdateChain {
    UpdateChain {
        steps: vec![UpdateStep::FullDownload {
            version: to_version.to_string(),
            url: platform.full_url.clone(),
            sha256: platform.full_sha256.clone(),
            size_bytes: platform.full_size_bytes,
        }],
        has_full_download: true,
        total_download_bytes: platform.full_size_bytes,
    }
}
