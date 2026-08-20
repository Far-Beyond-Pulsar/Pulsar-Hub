use gpui::prelude::*;
use gpui::*;
use ui::{
    button::Button, button::ButtonVariants as _, h_flex, v_flex, ActiveTheme as _, Icon, IconName,
};

use crate::screen::EntryScreen;
use super::helpers::render_page_header;

pub fn render_git_ci_tab(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let Some(ref settings) = screen.state.ui.project_settings else {
        return div().into_any_element();
    };
    let files = settings.workflow_files.clone();
    let workflow_dir = settings.project_path.join(".github").join("workflows");

    v_flex()
        .w_full()
        .gap_4()
        .child(render_page_header(
            IconName::Github,
            "Git CI/CD",
            format!("{} workflow file(s) found in .github/workflows", files.len()),
            cx,
        ))
        .child(v_flex().w_full().gap_2().children(files.iter().map(|file| {
            let file_clone = file.clone();
            let workflow_path = workflow_dir.join(&file_clone);
            h_flex()
                .w_full()
                .p_3()
                .gap_3()
                .items_center()
                .rounded_md()
                .bg(theme.secondary.opacity(0.08))
                .child(
                    Icon::new(IconName::List)
                        .size(px(16.))
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(theme.foreground)
                        .child(file.clone()),
                )
                .child(
                    Button::new(SharedString::from(format!("open-workflow-{}", file)))
                        .compact()
                        .ghost()
                        .label("Open")
                        .on_click(cx.listener(move |_, _, _, _cx| {
                            let _ = open::that(&workflow_path);
                        })),
                )
        })))
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("create-workflow")
                        .label("Create Workflow")
                        .primary()
                        .compact()
                        .on_click(cx.listener(|_, _, _, _cx| {})),
                )
                .child(
                    Button::new("open-workflows-dir")
                        .label("Open Workflows Folder")
                        .ghost()
                        .compact()
                        .on_click(cx.listener(move |_, _, _, _cx| {
                            let _ = open::that(&workflow_dir);
                        })),
                ),
        )
        .into_any_element()
}
