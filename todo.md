# To Do Before Merge

## BUGS

-   [ ] **Group management > Subscriptions:** Current subscriptions are not loaded
-   [ ] **Group management:** When clicking on a group without members, the link doesn't work
-   [ ] **User management > User:** User Type dropdown is not displaying the user's type
-   [ ] **User management > User:** User Group dropdown is not displaying the user's group
-   [ ] **Login after switing user_type:** After switing new user from user to group, the login goes to /portal/group_admin causing Error 500, stuck in loop. Unable to workaround
-   [ ] **Workbench:** I can't open or terminate anymore, so I can't check if the UI was completed
-   [ ] **Just command:** I can't run build-front because of uglifyjs ... – I was able to make it work by prepending npx, but you requested not to add that, how then?

## MAIN

-   [ ] **Subscriptions:** Added placeholder categories, API should fetch these from services.toml – search for // Placeholder
    -   Logic for what to display in the tray lives in subscription_details.html
    -   We may need different tray content for any additional categories
-   [ ] **System Management:** Split group & user management into two separate pages
-   [ ] **Group management:** Load groups by default
-   [ ] **Group management > User overview:** Add form to add user
-   [ ] **Group management > Create Group:** Instead of "Group updated" message, go to new group detail page
-   [ ] **Group management > Edit User:** Clear "User x edited" message after 2 seconds (otherwise no more feedback when you edit again)
-   [ ] **Group management > Delete User:** Instead of "User x deleted", return to list
-   [ ] Review time_ago_filter (vibe coded)

## WISH LIST

-   [ ] **Group management:** Ideally Members/Subscriptions would be two tabs under a single group detail page
    -   I would like to get rid of columns layout for all pages except homepage
-   [ ] **User management:** Ideally list of all users would be loaded on landing page
-   [ ] **User management:** Would it be possible to create a user before they sign up? It would make onboarding easier
-   [ ] **User management:** Instead of 1 second delay, can we implement a debounce?
-   [ ] **User database:** created_at and updated_at are stored as array instead of data, is that on purpose?
