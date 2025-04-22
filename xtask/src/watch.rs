#[derive(Debug)]
pub struct PathChangedFilterer;

impl watchexec::filter::Filterer for PathChangedFilterer {
    fn check_event(
        &self,
        event: &watchexec_events::Event,
        priority: watchexec_events::Priority,
    ) -> Result<bool, watchexec::error::RuntimeError> {
        if priority == watchexec_events::Priority::Urgent {
            return Ok(true);
        }

        // If any tag is a Keyboard, Process, Signal, or ProcessCompletion, then return true
        if event.tags.iter().any(|tag| {
            matches!(
                tag,
                watchexec_events::Tag::Keyboard(_)
                    | watchexec_events::Tag::Process(_)
                    | watchexec_events::Tag::Signal(_)
                    | watchexec_events::Tag::ProcessCompletion(_)
            )
        }) {
            return Ok(true);
        }

        // If any tag is a Path, then check that it is a modify, create, or delete operation
        if event
            .tags
            .iter()
            .any(|tag| matches!(tag, watchexec_events::Tag::Path { .. }))
        {
            for tag in event.tags.iter() {
                if let watchexec_events::Tag::FileEventKind(file_event_kind) = tag {
                    if file_event_kind.is_create()
                        || file_event_kind.is_modify()
                        || file_event_kind.is_remove()
                    {
                        return Ok(true);
                    }
                }
            }
            return Ok(false);
        }
        Ok(true)
    }
}
