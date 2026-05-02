#[cfg(test)]
mod tests {
    use crossbeam_channel::unbounded;

    #[test]
    fn closed_channel_send_error_is_handled_without_panic() {
        let (sender, receiver) = unbounded::<String>();
        drop(receiver); // close the channel
        // Sending should return an error, not panic
        let result = sender.send(String::from("test"));
        assert!(result.is_err(), "Sending on a closed channel should error");
    }
}
