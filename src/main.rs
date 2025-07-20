// from tutorial at:
//      https://book.iced.rs/first-steps.html

use iced::widget::{button, text, column, Column};

#[derive(Default)]
struct Counter {
    value: i64,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Increment,
    Decrement,
}

impl Counter {

    fn update(&mut self, message: Message) {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
    }

    fn view(&self) -> Column<Message> {
        let increment = button("+").on_press(Message::Increment);
        let decrement = button("-").on_press(Message::Decrement);
        let counter = text(self.value);
        let interface = column![increment, counter, decrement];
        interface
    }
}

pub fn main() -> iced::Result {
    iced::run("Counter", Counter::update, Counter::view)
}


#[test]
fn test_counter_update_01() {
    let mut counter = Counter { value: 0 };
    counter.update(Message::Increment);
    assert_eq!(counter.value, 1);
}
