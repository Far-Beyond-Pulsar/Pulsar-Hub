use gpui::prelude::*;
use gpui::*;
use ui::{
    button::Button, button::ButtonVariants as _, h_flex, v_flex, ActiveTheme as _,
    Disableable as _, IconName,
};
use wgpui_motion::prelude::*;

use crate::screen::EntryScreen;

pub fn render_home_banner(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let slides = screen.state.home_slides.clone();
    let count = slides.len();
    let index = screen.state.home_slide_index.min(count.saturating_sub(1));
    let slide = slides.get(index).cloned();
    let background = slide.as_ref().and_then(|slide| slide.image.clone());
    let kind = slide
        .as_ref()
        .map(|slide| slide.kind.clone())
        .unwrap_or_else(|| "Pulsar Engine".into());
    let title = slide
        .as_ref()
        .map(|slide| slide.title.clone())
        .unwrap_or_else(|| {
            if screen.state.home_feed_loading {
                "Loading the latest from Pulsar…".into()
            } else {
                "Build something remarkable.".into()
            }
        });
    let description = slide
        .as_ref()
        .map(|slide| slide.description.clone())
        .unwrap_or_else(|| {
            if screen.state.home_feed_loading {
                "Recent articles and engine releases will appear here.".into()
            } else {
                "Install an engine version to start creating a project.".into()
            }
        });
    let date = slide
        .as_ref()
        .map(|slide| slide.date.clone())
        .unwrap_or_default();
    let url = slide.as_ref().map(|slide| slide.url.clone());
    let total_slides = count.max(1);
    let needs_engine = screen.state.versions.installed.is_empty();
    let is_release = slide
        .as_ref()
        .is_some_and(|slide| slide.kind.to_lowercase().contains("release"));
    let show_install_action = is_release || slide.is_none();

    h_flex()
        .id("home-feature-carousel")
        .relative()
        .w_full()
        .h(px(330.))
        .flex_shrink_0()
        .overflow_hidden()
        .bg(theme.background.opacity(0.35))
        .when_some(background, |this, image| {
            this.child(
                img(ImageSource::Render(image))
                    .id(("home-slide-image", index))
                    .absolute()
                    .inset_0()
                    .w_full()
                    .h_full()
                    .object_fit(ObjectFit::Cover)
                    .motion(("home-slide-image-motion", index))
                    .timeline(presets::fade_in(180.ms())),
            )
        })
        .child(div().absolute().inset_0().bg(gpui::linear_gradient(
            0.,
            gpui::gradient_color_stop(gpui::hsla(0., 0., 0.08, 0.94), 0.),
            gpui::gradient_color_stop(gpui::hsla(0., 0., 0.05, 0.15), 1.),
        )))
        .child(
            v_flex()
                .id(("home-slide-copy", index))
                .relative()
                .w(px(640.))
                .max_w(px(740.))
                .h_full()
                .justify_center()
                .px_12()
                .py_8()
                .gap_2()
                .motion(("home-slide-copy-motion", index))
                .transition(Transition::token(MotionToken::SpatialFast))
                .enter(|style| style.opacity(0.).mt(px(6.)))
                .base(|style| style.opacity(1.).mt(px(0.)))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(gpui::hsla(0., 0., 1., 0.82))
                        .child(if date.is_empty() {
                            kind
                        } else {
                            format!("{kind}  ·  {date}")
                        }),
                )
                .child(
                    div()
                        .text_2xl()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(gpui::hsla(0., 0., 1., 1.))
                        .child(title),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::hsla(0., 0., 1., 0.88))
                        .max_w(px(580.))
                        .child(description),
                )
                .when_some(url, |this, url| {
                    this.child(
                        Button::new("home-slide-open")
                            .label(if is_release {
                                "View release"
                            } else {
                                "Read article"
                            })
                            .compact()
                            .ghost()
                            .text_color(gpui::hsla(0., 0., 1., 1.))
                            .on_click(move |_, _, _| {
                                let _ = open::that(url.clone());
                            }),
                    )
                })
                .when(show_install_action, |this| {
                    this.child(
                        Button::new("home-install-engine")
                            .label(if needs_engine {
                                "Install Engine"
                            } else {
                                "Install another version"
                            })
                            .icon(IconName::Download)
                            .compact()
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.ui.view = crate::core::types::EntryScreenView::Versions;
                                this.state.ui.show_install_modal = true;
                                if this.state.versions.available_releases.is_empty() {
                                    this.refresh_versions(cx);
                                }
                                cx.notify();
                            })),
                    )
                }),
        )
        .child(
            h_flex()
                .absolute()
                .inset_0()
                .items_center()
                .justify_between()
                .px_3()
                .child(
                    Button::new("home-slide-previous")
                        .icon(IconName::ChevronLeft)
                        .compact()
                        .ghost()
                        .text_color(gpui::hsla(0., 0., 1., 1.))
                        .disabled(count < 2)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if count > 0 {
                                this.state.home_slide_index =
                                    (this.state.home_slide_index + count - 1) % count;
                                cx.notify();
                            }
                        })),
                )
                .child(
                    Button::new("home-slide-next")
                        .icon(IconName::ChevronRight)
                        .compact()
                        .ghost()
                        .text_color(gpui::hsla(0., 0., 1., 1.))
                        .disabled(count < 2)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if count > 0 {
                                this.state.home_slide_index =
                                    (this.state.home_slide_index + 1) % count;
                                cx.notify();
                            }
                        })),
                ),
        )
        .child(
            h_flex()
                .absolute()
                .bottom_4()
                .left_0()
                .right_0()
                .justify_center()
                .gap_1p5()
                .children((0..total_slides).map(|slide_index| {
                    let active = slide_index == index;
                    div()
                        .id(format!("home-slide-dot-{slide_index}"))
                        .w(px(if active { 18. } else { 7. }))
                        .h(px(7.))
                        .rounded_full()
                        .bg(gpui::hsla(0., 0., 1., if active { 0.95 } else { 0.52 }))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.home_slide_index = slide_index;
                            cx.notify();
                        }))
                })),
        )
}
