struct GroupContent {
    items: Vec<String>,
}

impl GroupContent {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn add(&mut self, item: String) {
        self.items.push(item);
    }

    fn render(&self) -> String {
        let mut content = String::new();
        for item in &self.items {
            content.push_str(item);
        }
        content
    }
}

pub(super) fn get_group_content(subject: &str) -> String {
    format!("
    <div class='container mt-2'>
        <h1 class='lead'>create or modify a group</h1>
        <form id='form-group-form' hx-post='/submit' hx-target='#response' hx-swap='innerhtml'>
            <div class='form-group'>
                <label for='name'>name:</label>
                <input type='text' id='name' name='name' class='form-control' required>
                <div class='invalid-feedback'>please enter a name.</div>
            </div>
            <div class='form-group'>
                <label for='subscriptions'>subscriptions:</label>
                <select id='subscriptions' name='subscriptions' class='form-control' multiple required>
                    <option value='newsletter'>newsletter</option>
                    <option value='updates'>updates</option>
                    <option value='promotions'>promotions</option>
                    <option value='events'>events</option>
                </select>
                <div class='invalid-feedback'>please select at least one subscription.</div>
            </div>
            <div class='form-group'>
                <label for='last_updated_by'>last updated by:</label>
                <input type='text' id='last_updated_by' name='last_updated_by' class='form-control' value='{}' readonly>
            </div>
            <button type='submit' class='btn btn-primary'>submit</button>
        </form>
        <div id='response' class='mt-3'></div>
    </div>
    ", subject)
}
