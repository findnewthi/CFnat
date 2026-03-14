use std::sync::Arc;

pub fn update_ewma(current: &mut f32, new_val: f32, is_first: bool, alpha: f32) {
    if is_first {
        *current = new_val;
    } else {
        *current = (*current * (1.0 - alpha)) + (new_val * alpha);
    }
}

fn calculate_pool_avg<F>(backends: &[Arc<crate::backend::Backend>], get_value: F, default: f32) -> f32
where
    F: Fn(&crate::backend::Backend) -> f32,
{
    let mut total = 0.0;
    let mut total_samples = 0;

    for backend in backends {
        let sample_count = backend.get_sample_count();
        if sample_count > 0 {
            total += get_value(backend) * sample_count as f32;
            total_samples += sample_count;
        }
    }

    if total_samples == 0 {
        default
    } else {
        total / total_samples as f32
    }
}

pub fn calculate_pool_avg_delay(backends: &[Arc<crate::backend::Backend>]) -> f32 {
    calculate_pool_avg(backends, |b| b.get_avg_delay(), 1.0)
}

pub fn calculate_pool_avg_loss(backends: &[Arc<crate::backend::Backend>]) -> f32 {
    calculate_pool_avg(backends, |b| b.get_loss_rate(), 0.0)
}