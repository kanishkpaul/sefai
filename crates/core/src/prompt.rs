use crate::message::{Message, Role};

/// Everything the model will see for one turn, before chat-template rendering.
///
/// The harness supplies the conversation and ordered system sections.
/// The conversation is immutable after construction.
#[derive(Clone, Debug, Default)]
pub struct Prompt {
    pub(crate) sections: Vec<Section>,
    conversation: Vec<Message>,
}

/// A block of system text and its stable source name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub source: &'static str,
    pub text: String,
}

impl Prompt {
    /// Starts a prompt from the conversation so far, which normally ends with
    /// the user message being answered.
    pub fn new(conversation: Vec<Message>) -> Self {
        Prompt {
            sections: Vec::new(),
            conversation,
        }
    }

    /// Adds a system section. Sections render in the order they are added.
    pub fn add_section(&mut self, source: &'static str, text: impl Into<String>) {
        self.sections.push(Section {
            source,
            text: text.into(),
        });
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    pub fn conversation(&self) -> &[Message] {
        &self.conversation
    }

    /// Flattens the prompt into the message list a chat template expects.
    ///
    /// All non-blank sections join into one leading system message, separated
    /// by blank lines, because many chat templates accept only one system
    /// message and some reject one that is not first. Existing system messages
    /// follow sections in conversation order and join the same leading message.
    /// Blank system text is omitted; other roles and their content stay intact.
    pub fn to_messages(&self) -> Vec<Message> {
        let system = self
            .sections
            .iter()
            .map(|section| section.text.trim())
            .chain(
                self.conversation
                    .iter()
                    .filter(|message| message.role == Role::System)
                    .map(|message| message.content.trim()),
            )
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        let mut messages = Vec::with_capacity(self.conversation.len() + 1);
        if !system.is_empty() {
            messages.push(Message::system(system));
        }
        messages.extend(
            self.conversation
                .iter()
                .filter(|message| message.role != Role::System)
                .cloned(),
        );
        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Role;

    #[test]
    fn sections_merge_into_one_leading_system_message() {
        let mut prompt = Prompt::new(vec![Message::user("hi")]);
        prompt.add_section("persona", "You are Aria.");
        prompt.add_section("memory", "  ");
        prompt.add_section("memory", "They like rain.\n");

        let messages = prompt.to_messages();

        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0],
            Message::system("You are Aria.\n\nThey like rain.")
        );
        assert_eq!(messages[1].role, Role::User);
    }

    #[test]
    fn existing_system_text_joins_sections_once() {
        let mut prompt = Prompt::new(vec![
            Message::system("existing"),
            Message::user("<system>literal</system>"),
        ]);
        prompt.add_section("system", "section");
        assert_eq!(
            prompt.to_messages(),
            vec![
                Message::system("section\n\nexisting"),
                Message::user("<system>literal</system>")
            ]
        );
    }

    #[test]
    fn blank_system_text_is_omitted() {
        let prompt = Prompt::new(vec![Message::system(" \n"), Message::user("hi")]);
        assert_eq!(prompt.to_messages(), vec![Message::user("hi")]);
    }

    #[test]
    fn no_sections_means_no_system_message() {
        let prompt = Prompt::new(vec![Message::user("hi")]);

        assert_eq!(prompt.to_messages(), vec![Message::user("hi")]);
    }
}
