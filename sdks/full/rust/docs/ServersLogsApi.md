# \ServersLogsApi

All URIs are relative to *https://api.rivet.gg*

Method | HTTP request | Description
------------- | ------------- | -------------
[**servers_logs_get**](ServersLogsApi.md#servers_logs_get) | **GET** /games/{game_id}/environments/{environment_id}/servers/{server_id}/logs | 



## servers_logs_get

> crate::models::ServersGetServerLogsResponse servers_logs_get(game_id, environment_id, server_id, stream, watch_index)


Returns the logs for a given server.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**game_id** | **uuid::Uuid** |  | [required] |
**environment_id** | **uuid::Uuid** |  | [required] |
**server_id** | **uuid::Uuid** |  | [required] |
**stream** | [**ServersLogStream**](.md) |  | [required] |
**watch_index** | Option<**String**> | A query parameter denoting the requests watch index. |  |

### Return type

[**crate::models::ServersGetServerLogsResponse**](ServersGetServerLogsResponse.md)

### Authorization

[BearerAuth](../README.md#BearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

