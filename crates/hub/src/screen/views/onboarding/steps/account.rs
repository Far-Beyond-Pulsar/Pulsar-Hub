use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, ActiveTheme, Icon, IconName, StyledExt,
};

use crate::screen::EntryScreen;
use crate::service::auth_service::AuthService;

pub fn render(screen: &mut EntryScreen, cx: &mut Context<EntryScreen>) -> AnyElement {
    let theme = cx.theme().clone();
    let profile = AuthService::profile();
    let code = screen.state.auth.device_code.clone();
    let message = screen.state.auth.message.clone();
    let loading = screen.state.auth.loading;
    let signed_in = profile.is_some();

    v_flex()
        .size_full()
        .gap_4()
        .child(
            h_flex()
                .items_center()
                .gap_3()
                .child(Icon::new(IconName::Group).size_6().text_color(theme.accent))
                .child(v_flex().gap_1()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(theme.foreground).child("Your account"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Sign in to sync preferences and collaborate. This is optional; you can connect an account later."))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .justify_center()
                .items_center()
                .gap_4()
                .p_6()
                .bg(theme.secondary.opacity(0.12))
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .when_some(profile, |this, profile| {
                    let display_name = profile.display_name.unwrap_or_else(|| profile.login.clone());
                    this.child(Icon::new(IconName::Check).size_8().text_color(theme.success_foreground))
                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(theme.foreground).child(format!("Signed in as {display_name}")))
                        .child(div().text_sm().text_color(theme.muted_foreground).child(format!("@{} · sync and collaboration are ready", profile.login)))
                })
                .when(!signed_in && !loading && code.is_none(), |this| {
                    this.child(Icon::new(IconName::Group).size_8().text_color(theme.muted_foreground))
                        .child(div().max_w(px(520.)).text_center().text_sm().text_color(theme.muted_foreground)
                            .child("Connect GitHub to enable cloud sync and multiplayer features. Pulsar Hub will open a browser to complete sign-in."))
                        .child(Button::new("signin-github-onboarding").label("Connect GitHub").primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.begin_github_sign_in(cx);
                                cx.notify();
                            })))
                })
                .when(loading, |this| {
                    this.child(div().text_sm().text_color(theme.muted_foreground).child("Waiting for GitHub sign-in…"))
                })
                .when_some(code, |this, code| {
                    this.child(div().text_sm().text_color(theme.foreground).child("Enter this one-time code in the browser to finish signing in:"))
                        .child(div().px_6().py_3().rounded_lg().bg(theme.accent.opacity(0.12))
                            .text_2xl().font_weight(FontWeight::BOLD).text_color(theme.foreground).child(code))
                })
                .when_some(message, |this, message| {
                    this.child(div().max_w(px(520.)).text_center().text_sm().text_color(theme.muted_foreground).child(message))
                })
                .child(div().text_xs().text_color(theme.muted_foreground).child("No account is required to install an engine or use local projects.")),
        )
        .into_any_element()
}
