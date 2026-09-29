use crate::core::types::OnboardingStep;

#[derive(Clone, Copy)]
pub struct StepDefinition {
    pub step: OnboardingStep,
    pub title: &'static str,
}

/// The ordered onboarding flow. Screen order and stepper labels live together
/// so adding a step does not require parallel index lists in the renderer.
pub const ONBOARDING_STEPS: [StepDefinition; 6] = [
    StepDefinition {
        step: OnboardingStep::Welcome,
        title: "Welcome",
    },
    StepDefinition {
        step: OnboardingStep::Engine,
        title: "First engine",
    },
    StepDefinition {
        step: OnboardingStep::Dependencies,
        title: "Build tools",
    },
    StepDefinition {
        step: OnboardingStep::Account,
        title: "Account",
    },
    StepDefinition {
        step: OnboardingStep::Theme,
        title: "Theme",
    },
    StepDefinition {
        step: OnboardingStep::Plugins,
        title: "Plugins",
    },
];

pub fn step_index(step: OnboardingStep) -> usize {
    ONBOARDING_STEPS
        .iter()
        .position(|definition| definition.step == step)
        .unwrap_or(0)
}

impl OnboardingStep {
    pub fn next(self) -> Option<Self> {
        ONBOARDING_STEPS
            .get(step_index(self) + 1)
            .map(|definition| definition.step)
    }

    pub fn previous(self) -> Option<Self> {
        step_index(self)
            .checked_sub(1)
            .map(|index| ONBOARDING_STEPS[index].step)
    }
}
